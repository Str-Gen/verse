#include <QGuiApplication>
#include <QString>
#include <QTextDocument>
#include <iostream>

int main(int argc, char **argv) {
    QGuiApplication app(argc, argv);
    int failures = 0;
    const auto check = [&failures](const char *name, const QString &html,
                                  const QString &expected) {
        QTextDocument document;
        document.setHtml(html);
        if (document.toPlainText() != expected) {
            std::cerr << "FAIL: " << name << '\n';
            ++failures;
        }
    };
    check("pre-wrap whitespace",
          "<p style=\"white-space: pre-wrap;\">a  b\tc\nd\re\r\nf</p><p>next</p>",
          "a  b\tc\nd\ne\n\nf\nnext");
    check("explicit break", "<p>a<br/>b</p><p>next</p>", "a\nb\nnext");
    check("numeric whitespace entities",
          "<p style=\"white-space: pre-wrap;\">a&#9;b&#10;c&#13;d</p><p>next</p>",
          "a\tb\nc\nd\nnext");
    check("empty paragraphs disappear",
          "<p></p><p></p><p>next</p><p></p><p>last</p>", "next\nlast");
    check("whitespace-only paragraph survives",
          "<p style=\"white-space: pre-wrap;\">   </p><p>next</p>", "   \nnext");
    check("Unicode plain text conversion",
          QString::fromUtf8("<p style=\"white-space: pre-wrap;\">a\xc2\xa0"
                            "b\xe2\x80\xa8" "c\xe2\x80\xa9" "d</p>"),
          "a b\nc\nd");
    check("leading source newline in a heading",
          "<p>Before</p><h2 style=\"white-space: pre-wrap;\"><br/>Heading</h2><p>After</p>",
          "Before\n\nHeading\nAfter");
    if (failures == 0) {
        std::cout << "PASS: seven native QTextDocument whitespace checks\n";
    }
    return failures == 0 ? 0 : 1;
}
