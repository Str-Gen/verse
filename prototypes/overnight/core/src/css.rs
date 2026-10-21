//! Bounded author CSS cascade for content visibility and inline semantics.
//! Unsupported selectors/rules fail closed rather than silently exposing hidden text.
use cssparser::{Parser, ParserInput, Token};
use roxmltree::Node;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Style {
    pub display_none: bool,
    pub visible: bool,
    pub emphasis: bool,
    pub strong: bool,
}
impl Default for Style {
    fn default() -> Self {
        Self {
            display_none: false,
            visible: true,
            emphasis: false,
            strong: false,
        }
    }
}
#[derive(Clone, Debug)]
enum Atom {
    Tag(String),
    Class(String),
    Id(String),
    Universal,
}
#[derive(Clone, Copy, Debug)]
enum Relation {
    Descendant,
    Child,
}
#[derive(Clone, Debug)]
struct Selector {
    compounds: Vec<Vec<Atom>>,
    relations: Vec<Relation>,
    specificity: (usize, usize, usize),
}
#[derive(Clone, Debug)]
struct Declaration {
    property: usize,
    value: Value,
    important: bool,
}
#[derive(Clone, Copy, Debug)]
enum Value {
    Bool(bool),
    Inherit,
    Initial,
}
#[derive(Clone, Debug)]
struct Rule {
    selectors: Vec<Selector>,
    declarations: Vec<Declaration>,
}
#[derive(Clone, Debug, Default)]
pub struct Css {
    rules: Vec<Rule>,
}
#[derive(Clone, Debug)]
enum Lex {
    Ident(String),
    Hash(String),
    Delim(char),
    Space,
    Number(i32),
}

fn error(message: &str) -> String {
    format!("unsupported or malformed CSS: {message}")
}
fn nested_skip(input: &mut Parser<'_, '_>, depth: usize) -> Result<(), String> {
    if depth > 64 {
        return Err(error("nested CSS value limit"));
    }
    while !input.is_exhausted() {
        let token = input
            .next_including_whitespace_and_comments()
            .map_err(|_| error("token"))?
            .clone();
        match token {
            Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock => {
                input
                    .parse_nested_block(|p| {
                        nested_skip(p, depth + 1)
                            .map_err(|e| p.new_custom_error::<String, String>(e))
                    })
                    .map_err(|_| error("nested value"))?;
            }
            Token::BadString(_)
            | Token::BadUrl(_)
            | Token::CloseParenthesis
            | Token::CloseSquareBracket
            | Token::CloseCurlyBracket => return Err(error("invalid token")),
            _ => {}
        }
    }
    Ok(())
}
fn lex(token: Token<'_>) -> Result<Option<Lex>, String> {
    Ok(match token {
        Token::Ident(s) => Some(Lex::Ident(s.to_string())),
        Token::IDHash(s) | Token::Hash(s) => Some(Lex::Hash(s.to_string())),
        Token::Delim(c) => Some(Lex::Delim(c)),
        Token::WhiteSpace(_) => Some(Lex::Space),
        Token::Comment(_) => None,
        Token::Number {
            int_value: Some(n), ..
        } => Some(Lex::Number(n)),
        _ => return Err(error("selector/value token")),
    })
}
fn selector(tokens: &[Lex]) -> Result<Selector, String> {
    let mut compounds = Vec::new();
    let mut relations = Vec::new();
    let mut current = Vec::new();
    let mut pending = None;
    let mut specificity = (0, 0, 0);
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            Lex::Space => {
                if !current.is_empty() {
                    compounds.push(std::mem::take(&mut current));
                    pending = Some(Relation::Descendant);
                }
            }
            Lex::Delim('>') => {
                if !current.is_empty() {
                    compounds.push(std::mem::take(&mut current));
                }
                if compounds.is_empty() || matches!(pending, Some(Relation::Child)) {
                    return Err(error("child selector"));
                }
                pending = Some(Relation::Child);
            }
            token => {
                if current.is_empty() && !compounds.is_empty() {
                    relations.push(pending.take().ok_or_else(|| error("selector combinator"))?);
                }
                match token {
                    Lex::Ident(name) if current.is_empty() => {
                        current.push(Atom::Tag(name.clone()));
                        specificity.2 += 1;
                    }
                    Lex::Delim('*') if current.is_empty() => current.push(Atom::Universal),
                    Lex::Hash(id) => {
                        current.push(Atom::Id(id.clone()));
                        specificity.0 += 1;
                    }
                    Lex::Delim('.') => {
                        i += 1;
                        if let Some(Lex::Ident(class)) = tokens.get(i) {
                            current.push(Atom::Class(class.clone()));
                            specificity.1 += 1;
                        } else {
                            return Err(error("class selector"));
                        }
                    }
                    _ => {
                        return Err(error(
                            "only type, class, id, descendant and child selectors are supported",
                        ));
                    }
                }
            }
        }
        i += 1;
    }
    if !current.is_empty() {
        compounds.push(current);
    } else if matches!(pending, Some(Relation::Child)) {
        return Err(error("trailing child combinator"));
    }
    if compounds.is_empty() || relations.len() + 1 != compounds.len() {
        return Err(error("empty selector"));
    }
    Ok(Selector {
        compounds,
        relations,
        specificity,
    })
}
fn declarations(input: &mut Parser<'_, '_>) -> Result<Vec<Declaration>, String> {
    let mut result = Vec::new();
    while !input.is_exhausted() {
        if input.try_parse(|p| p.expect_semicolon()).is_ok() {
            continue;
        }
        let property = input
            .expect_ident_cloned()
            .map_err(|_| error("property name"))?
            .to_ascii_lowercase();
        input
            .expect_colon()
            .map_err(|_| error("missing declaration colon"))?;
        let relevant = match property.as_str() {
            "display" => Some(0),
            "visibility" => Some(1),
            "font-style" => Some(2),
            "font-weight" => Some(3),
            _ => None,
        };
        if matches!(
            property.as_str(),
            "content"
                | "opacity"
                | "clip"
                | "clip-path"
                | "text-transform"
                | "white-space"
                | "all"
                | "font"
                | "animation"
                | "animation-name"
                | "transition"
                | "-webkit-text-security"
        ) {
            return Err(error("content-changing property outside supported subset"));
        }
        let mut tokens = Vec::new();
        while !input.is_exhausted() {
            let token = input
                .next_including_whitespace_and_comments()
                .map_err(|_| error("declaration value"))?
                .clone();
            if matches!(token, Token::Semicolon) {
                break;
            }
            if relevant.is_some() {
                if let Some(t) = lex(token)?
                    && !matches!(t, Lex::Space)
                {
                    tokens.push(t);
                }
            } else {
                match token {
                    Token::Function(_)
                    | Token::ParenthesisBlock
                    | Token::SquareBracketBlock
                    | Token::CurlyBracketBlock => input
                        .parse_nested_block(|p| {
                            nested_skip(p, 0).map_err(|e| p.new_custom_error::<String, String>(e))
                        })
                        .map_err(|_| error("declaration nested value"))?,
                    Token::BadString(_)
                    | Token::BadUrl(_)
                    | Token::CloseParenthesis
                    | Token::CloseSquareBracket
                    | Token::CloseCurlyBracket => return Err(error("declaration token")),
                    _ => {}
                }
            }
        }
        let Some(property) = relevant else {
            continue;
        };
        let important = tokens.len() >= 2
            && matches!(&tokens[tokens.len() - 2], Lex::Delim('!'))
            && matches!(&tokens[tokens.len()-1], Lex::Ident(s) if s.eq_ignore_ascii_case("important"));
        if important {
            tokens.truncate(tokens.len() - 2);
        }
        let value = match tokens.as_slice() {
            [Lex::Ident(s)] if s.eq_ignore_ascii_case("inherit") => Value::Inherit,
            [Lex::Ident(s)] if s.eq_ignore_ascii_case("initial") => Value::Initial,
            [Lex::Ident(s)] if s.eq_ignore_ascii_case("unset") => {
                if property == 0 {
                    Value::Initial
                } else {
                    Value::Inherit
                }
            }
            [Lex::Ident(s)] => {
                let value = s.to_ascii_lowercase();
                Value::Bool(match (property, value.as_str()) {
                    (0, "none") => true,
                    (
                        0,
                        "block" | "inline" | "inline-block" | "list-item" | "table" | "table-row"
                        | "table-cell" | "flex" | "grid" | "contents",
                    ) => false,
                    (1, "visible") => true,
                    (1, "hidden" | "collapse") => false,
                    (2, "italic" | "oblique") => true,
                    (2 | 3, "normal") => false,
                    (3, "bold") => true,
                    _ => return Err(error("unsupported property value")),
                })
            }
            [Lex::Number(n)] if property == 3 && (1..=1000).contains(n) => Value::Bool(*n >= 600),
            _ => return Err(error("property requires one supported keyword or weight")),
        };
        result.push(Declaration {
            property,
            value,
            important,
        });
    }
    Ok(result)
}
fn page_declarations(input: &mut Parser<'_, '_>) -> Result<(), String> {
    while !input.is_exhausted() {
        if input.try_parse(|p| p.expect_semicolon()).is_ok() {
            continue;
        }
        let name = input
            .expect_ident_cloned()
            .map_err(|_| error("page property"))?;
        if !matches!(
            name.as_ref(),
            "margin"
                | "margin-top"
                | "margin-bottom"
                | "margin-left"
                | "margin-right"
                | "padding"
                | "padding-top"
                | "padding-bottom"
                | "padding-left"
                | "padding-right"
        ) {
            return Err(error(
                "only page margins and padding can be normalized away",
            ));
        }
        input
            .expect_colon()
            .map_err(|_| error("page declaration colon"))?;
        let mut values = 0;
        while !input.is_exhausted() {
            match input.next().map_err(|_| error("page margin value"))? {
                Token::Semicolon => break,
                Token::Number { .. } | Token::Dimension { .. } | Token::Percentage { .. } => {
                    values += 1
                }
                Token::Ident(value) if value.eq_ignore_ascii_case("auto") => values += 1,
                _ => return Err(error("only plain page margins are supported")),
            }
        }
        if !(1..=4).contains(&values) {
            return Err(error("page margin value count"));
        }
    }
    Ok(())
}

impl Css {
    pub fn parse(sheets: &[String]) -> Result<Self, String> {
        let mut rules = Vec::new();
        if sheets.len() > 1_000 {
            return Err(error("stylesheet count limit"));
        }
        for sheet in sheets {
            if sheet.len() > 1_048_576 {
                return Err(error("stylesheet byte limit"));
            }
            let mut source = ParserInput::new(sheet);
            let mut parser = Parser::new(&mut source);
            'rules: while !parser.is_exhausted() {
                let mut selectors = Vec::new();
                let mut tokens = Vec::new();
                loop {
                    let token = parser
                        .next_including_whitespace_and_comments()
                        .map_err(|_| error("missing rule body"))?
                        .clone();
                    match token {
                        Token::Comma => {
                            selectors.push(selector(&tokens)?);
                            tokens.clear();
                        }
                        Token::CurlyBracketBlock => break,
                        Token::AtKeyword(name)
                            if name.eq_ignore_ascii_case("page")
                                && tokens.iter().all(|t| matches!(t, Lex::Space)) =>
                        {
                            if !matches!(parser.next(), Ok(Token::CurlyBracketBlock)) {
                                return Err(error(
                                    "only unqualified page margin rules are supported",
                                ));
                            }
                            let start = parser.position();
                            parser
                                .parse_nested_block(|p| {
                                    page_declarations(p)
                                        .map_err(|e| p.new_custom_error::<String, String>(e))
                                })
                                .map_err(|_| error("invalid or unsupported page rule"))?;
                            if !parser.slice_from(start).ends_with('}') {
                                return Err(error("unclosed page rule"));
                            }
                            continue 'rules;
                        }
                        Token::AtKeyword(_) => {
                            return Err(error("at-rules require an explicit rendering policy"));
                        }
                        token => {
                            if let Some(t) = lex(token)? {
                                tokens.push(t);
                            }
                        }
                    }
                }
                selectors.push(selector(&tokens)?);
                let start = parser.position();
                let declarations = parser
                    .parse_nested_block(|p| {
                        declarations(p).map_err(|e| p.new_custom_error::<String, String>(e))
                    })
                    .map_err(|_| error("invalid or unsupported declaration"))?;
                if !parser.slice_from(start).ends_with('}') {
                    return Err(error("unclosed rule"));
                }
                rules.push(Rule {
                    selectors,
                    declarations,
                });
                if rules.len() > 10_000 {
                    return Err(error("rule limit exceeded"));
                }
            }
        }
        Ok(Self { rules })
    }
    pub fn style(&self, node: Node<'_, '_>) -> Result<Style, String> {
        let mut style = Style::default();
        let mut ancestors: Vec<_> = node.ancestors().filter(Node::is_element).collect();
        ancestors.reverse();
        if ancestors.len() > 256 {
            return Err(error("element nesting limit"));
        }
        for element in ancestors {
            let parent = style;
            let mut values = [false, parent.visible, parent.emphasis, parent.strong];
            match element.tag_name().name() {
                "em" | "i" => values[2] = true,
                "strong" | "b" => values[3] = true,
                "head" | "script" | "style" | "template" => values[0] = true,
                _ => {}
            }
            type Priority = (bool, bool, (usize, usize, usize), usize);
            let mut priorities: [Option<Priority>; 4] = [None; 4];
            let mut order = 0;
            let mut apply = |declaration: &Declaration, inline, specificity| {
                order += 1;
                let priority = (declaration.important, inline, specificity, order);
                let property = declaration.property;
                if priorities[property].is_none_or(|old| priority >= old) {
                    priorities[property] = Some(priority);
                    values[property] = match declaration.value {
                        Value::Bool(value) => value,
                        Value::Initial => property == 1,
                        Value::Inherit => [
                            parent.display_none,
                            parent.visible,
                            parent.emphasis,
                            parent.strong,
                        ][property],
                    };
                }
            };
            for rule in &self.rules {
                if let Some(specificity) = rule
                    .selectors
                    .iter()
                    .filter(|s| s.matches(element))
                    .map(|s| s.specificity)
                    .max()
                {
                    for declaration in &rule.declarations {
                        apply(declaration, false, specificity);
                    }
                }
            }
            if let Some(inline) = element.attribute("style") {
                if inline.len() > 65_536 {
                    return Err(error("inline style byte limit"));
                }
                let mut source = ParserInput::new(inline);
                for declaration in declarations(&mut Parser::new(&mut source))? {
                    apply(&declaration, true, (0, 0, 0));
                }
            }
            style = Style {
                display_none: parent.display_none || values[0] || element.has_attribute("hidden"),
                visible: values[1],
                emphasis: values[2],
                strong: values[3],
            };
        }
        Ok(style)
    }
}
impl Selector {
    fn matches(&self, node: Node<'_, '_>) -> bool {
        self.matches_at(node, self.compounds.len() - 1)
    }
    fn matches_at(&self, node: Node<'_, '_>, index: usize) -> bool {
        if !self.compounds[index].iter().all(|atom| match atom {
            Atom::Universal => true,
            Atom::Tag(tag) => node.tag_name().name() == tag,
            Atom::Id(id) => node.attribute("id") == Some(id.as_str()),
            Atom::Class(class) => node.attribute("class").is_some_and(|classes| {
                classes.split_ascii_whitespace().any(|value| value == class)
            }),
        }) {
            return false;
        }
        if index == 0 {
            return true;
        }
        match self.relations[index - 1] {
            Relation::Child => node
                .parent_element()
                .is_some_and(|parent| self.matches_at(parent, index - 1)),
            Relation::Descendant => node
                .ancestors()
                .skip(1)
                .filter(Node::is_element)
                .any(|parent| self.matches_at(parent, index - 1)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn visible_text(xml: &str, css: &str) -> Result<String, String> {
        let document = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
        let css = Css::parse(&[css.to_owned()])?;
        let mut text = String::new();
        for node in document.descendants().filter(Node::is_text) {
            let style = css.style(node)?;
            if style.visible && !style.display_none {
                text.push_str(node.text().unwrap_or_default());
            }
        }
        Ok(text)
    }
    #[test]
    fn ancestor_display_suppresses_but_visibility_can_be_restored() -> Result<(), String> {
        assert_eq!(
            visible_text(
                "<body><p class='gone'>secret<b style='display:block'>also secret</b></p><p class='invisible'>hidden<i style='visibility:visible'>shown</i></p></body>",
                ".gone {display:none}.invisible {visibility:hidden}"
            )?,
            "shown"
        );
        Ok(())
    }
    #[test]
    fn cascade_has_specificity_order_inline_and_important() -> Result<(), String> {
        let xml = "<body><p id='x' class='gone' style='display:none'>shown</p><div><p class='gone'>hidden</p></div></body>";
        assert_eq!(
            visible_text(
                xml,
                ".gone {display:none} body > p#x {display:block!important} #x {display:none}"
            )?,
            "shown"
        );
        Ok(())
    }
    #[test]
    fn descendant_and_compound_selectors() -> Result<(), String> {
        assert_eq!(
            visible_text(
                "<body><section><p class='a b'>hidden</p><p>shown</p></section></body>",
                "body section p.a.b {display:none}"
            )?,
            "shown"
        );
        Ok(())
    }
    #[test]
    fn only_plain_page_margins_are_normalized() -> Result<(), String> {
        assert_eq!(
            visible_text(
                "<body><p>shown</p></body>",
                "@page { margin-bottom: 5pt; margin-top: 5pt } @page { padding:0; margin:0 } p {display:block}"
            )?,
            "shown"
        );
        for css in [
            "@page { content:'secret' }",
            "@page { @top-center { content:'secret' } }",
            "@page :left {margin:0}",
            "@page {margin:0",
        ] {
            assert!(Css::parse(&[css.to_owned()]).is_err());
        }
        Ok(())
    }
    #[test]
    fn unsupported_or_malformed_css_is_explicit() {
        for css in [
            "p[hidden]{display:none}",
            "@media screen {p {display:none}}",
            "p {display:none",
            "p {display:none; visibility}",
            "p {opacity:0}",
            "p {font-weight:bolder}",
            "p {font-weight:lighter}",
            "p:hover{display:none}",
        ] {
            assert!(Css::parse(&[css.to_owned()]).is_err(), "accepted {css}");
        }
    }
    #[test]
    fn escaped_class_and_comments_use_css_tokenization() -> Result<(), String> {
        assert_eq!(
            visible_text(
                "<body><p class='hidden'>secret</p><p>shown</p></body>",
                ".h\\69 dden/* comment */ {display:/**/none}"
            )?,
            "shown"
        );
        Ok(())
    }
}
