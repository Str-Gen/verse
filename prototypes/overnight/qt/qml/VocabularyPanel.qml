import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Dialog {
    id: panel
    property var readerObject
    property var review: ({captures: [], suggestions: [], words: []})
    property string message: ""
    property string scope: "seconds"
    property bool practicing: false
    property bool definitionOpen: false
    property int practiceAt: 0
    property int comfortable: 0
    property int difficult: 0
    property int skipped: 0
    property string practiceWord: ""
    property string practiceContext: ""
    property string practiceDefinition: ""
    property bool practiceDone: practiceAt >= review.words.length
    title: practicing ? "Recognition practice" : "Word list"
    width: Math.min(parent.width - 40, 900)
    height: Math.min(parent.height - 60, 680)
    anchors.centerIn: parent
    modal: true
    standardButtons: Dialog.Close
    function request(value) {
        let response = JSON.parse(readerObject.vocabulary(JSON.stringify(value)))
        if (response.error) { message = response.error; return null }
        return response.result
    }
    function capture() {
        let result = request({action: "capture", scope: scope})
        if (result) message = "Captured. This book's word list is stored for later review · V."
    }
    function refresh() {
        let result = request({action: "review"})
        if (result) { review = result; message = result.captures.length + " captures · " + result.words.length + " saved words" }
    }
    function showReview() { readerObject.pause(); practicing = false; definitionOpen = false; refresh(); open() }
    function definitions(word) {
        let result = request({action: "definition", word: word})
        if (!result) return message
        if (!result.available) return "The local dictionary is not installed. Set VERSE_DICTIONARY to its SQLite file."
        if (!result.entries.length) return "This word is not in the local dictionary."
        return result.entries.map(entry => [entry.word + " · " + (entry.pos || ""), entry.etymology_text || "",
            ...entry.senses.map(sense => [...(sense.tags || []), ...(sense.raw_tags || []), ...(sense.glosses || [])].join(" · ")),
            "https://en.wiktionary.org/wiki/" + encodeURIComponent(entry.word)].filter(Boolean).join("\n")).join("\n\n")
            + "\n\nWiktionary contributors · Kaikki / Wiktextract · CC BY-SA 4.0\nhttps://creativecommons.org/licenses/by-sa/4.0/"
    }
    function escaped(text) { return text.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;") }
    function sentence(candidate) {
        if (!candidate.sentence) return escaped(candidate.word + " · Captured partial context: " + candidate.context)
        let chars = Array.from(candidate.sentence)
        return escaped(chars.slice(0,candidate.sentence_start).join("")) + "<b>" + escaped(chars.slice(candidate.sentence_start,candidate.sentence_end).join("")) + "</b>" + escaped(chars.slice(candidate.sentence_end).join(""))
    }
    function card() {
        flash.stop()
        practiceContext = ""; practiceDefinition = ""
        practiceWord = practiceDone ? "Session complete" : review.words[practiceAt].word
        if (!practiceDone) flash.start()
    }
    onClosed: { flash.stop(); definitionOpen = false }
    Timer { id: flash; interval: 60000 / panel.readerObject.speed; onTriggered: panel.practiceWord = "•••" }
    Timer { id: poll; interval: 1000; repeat: true; running: panel.visible && !panel.practicing && !panel.definitionOpen && panel.review.captures.some(c => c.status === "ranking"); onTriggered: panel.refresh() }
    contentItem: ColumnLayout {
        Label { text: panel.message; textFormat: Text.PlainText; wrapMode: Text.Wrap; Layout.fillWidth: true }
        ScrollView {
            id: reviewScroll
            visible: !panel.practicing
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            ColumnLayout {
                width: reviewScroll.availableWidth
                Repeater {
                    model: panel.review.captures.filter(c => c.status !== "ranked")
                    ColumnLayout {
                        required property var modelData
                        Layout.fillWidth: true
                        Label { text: modelData.error || "Ranking captured words…"; textFormat: Text.PlainText; wrapMode: Text.Wrap; Layout.fillWidth: true }
                        Button { visible: modelData.status !== "ranking"; text: "Retry ranking"; onClicked: { panel.request({action:"retry",capture:modelData.id}); panel.refresh() } }
                    }
                }
                Repeater {
                    model: panel.review.suggestions
                    Frame {
                        id: row
                        required property var modelData
                        property bool expanded: false
                        property string definition: ""
                        Layout.fillWidth: true
                        ColumnLayout {
                            width: parent.width
                            Label { text: panel.sentence(row.modelData.candidate); textFormat: Text.RichText; wrapMode: Text.Wrap; Layout.fillWidth: true }
                            Flow {
                                visible: !row.expanded
                                Layout.fillWidth: true
                                spacing: 6
                                Repeater {
                                    model: [{choice:"unfamiliar",label:"A · Unfamiliar"},{choice:"recognition",label:"B · Know it but slow to recognize"},{choice:"dismissed",label:"C · Dismiss"}]
                                    Button {
                                        required property var modelData
                                        text: modelData.label
                                        highlighted: row.modelData.choice === modelData.choice
                                        onClicked: { panel.request({action:"feedback",capture:row.modelData.capture,candidate:row.modelData.candidate_index,choice:modelData.choice}); panel.refresh() }
                                    }
                                }
                            }
                            Button { text: row.expanded ? "Collapse definition" : "See definition"; onClicked: { row.expanded = !row.expanded; panel.definitionOpen = row.expanded; if (row.expanded) { row.definition = panel.definitions(row.modelData.candidate.word) } } }
                            TextArea { visible: row.expanded; text: row.definition; textFormat: TextEdit.PlainText; readOnly: true; wrapMode: Text.Wrap; Layout.fillWidth: true; background: null }
                        }
                    }
                }
            }
        }
        ScrollView {
            id: practiceScroll
            visible: panel.practicing
            Layout.fillWidth: true
            Layout.fillHeight: true
            ColumnLayout {
                width: practiceScroll.availableWidth
                Label { text: panel.practiceWord; font.pixelSize: 40; horizontalAlignment: Text.AlignHCenter; Layout.fillWidth: true; textFormat: Text.PlainText }
                Label { text: panel.practiceContext; wrapMode: Text.Wrap; Layout.fillWidth: true; textFormat: Text.PlainText }
                TextArea { text: panel.practiceDefinition; readOnly: true; textFormat: TextEdit.PlainText; wrapMode: Text.Wrap; Layout.fillWidth: true; background: null }
                Button { text: "Reveal context and definitions"; enabled: !panel.practiceDone; onClicked: { let word = panel.review.words[panel.practiceAt]; panel.practiceContext = word.candidate.sentence || "Captured partial context: " + word.candidate.context; panel.practiceDefinition = panel.definitions(word.word) } }
                Flow {
                    Layout.fillWidth: true
                    Button { text: "Comfortable"; enabled: !panel.practiceDone; onClicked: { panel.comfortable++; panel.practiceAt++; panel.card() } }
                    Button { text: "Difficult"; enabled: !panel.practiceDone; onClicked: { panel.difficult++; panel.practiceAt++; panel.card() } }
                    Button { text: "Skip"; enabled: !panel.practiceDone; onClicked: { panel.skipped++; panel.practiceAt++; panel.card() } }
                }
                Label { text: "Comfortable " + panel.comfortable + " · Difficult " + panel.difficult + " · Skipped " + panel.skipped }
            }
        }
        Button {
            text: panel.practicing ? "Back to word list" : "Practice saved words"
            enabled: panel.practicing || panel.review.words.length > 0
            onClicked: {
                panel.practicing = !panel.practicing
                if (panel.practicing) { panel.practiceAt = 0; panel.comfortable = 0; panel.difficult = 0; panel.skipped = 0; panel.card() }
                else { flash.stop(); panel.refresh() }
            }
        }
    }
}
