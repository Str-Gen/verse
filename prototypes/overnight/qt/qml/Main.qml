import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import QtQuick.Shapes
import Overnight.Reader

ApplicationWindow {
    id: window
    width: 1000
    height: 780
    minimumWidth: 560
    minimumHeight: 480
    visible: true
    title: reader.title + " · Verse"
    color: theme.background
    palette.window: theme.background
    palette.base: theme.background
    palette.text: theme.foreground
    palette.windowText: theme.foreground
    palette.button: theme.background
    palette.buttonText: theme.foreground
    palette.highlight: theme.selection
    palette.highlightedText: theme.selectionForeground
    property bool playback: false
    property int textSize: 20
    property bool controlsVisible: false
    font.family: "IBM Plex Mono"
    font.pixelSize: 12
    FontMetrics { id: readingMetrics; font.family: "IBM Plex Mono"; font.pixelSize: window.textSize * theme.textScale }
    property var pages: [{ y: 0, position: 0 }]
    property int page: 0
    property bool ready: false
    property bool shortcutsActive: !openDialog.visible && !recoveryDialog.visible && !chooseLocationDialog.visible && !noteDialog.visible
    property real pageY: pages[Math.min(page, pages.length - 1)].y

    Reader { id: reader }
    function preserveSelection() {
        if (ready && !playback && editor.selectedText.length > 0) reader.select(editor.selectionStart)
    }
    function paginate() {
        if (!ready || viewport.height < 100 || editor.contentHeight <= 0) return
        support.fitImages(editor, Math.max(1, editor.width - 8), Math.max(1, viewport.height - editor.font.pixelSize * 3))
        let result = [{y: 0, position: 0}]
        let y = 0
        while (y + viewport.height < editor.contentHeight) {
            let position = editor.positionAt(0, y + viewport.height - editor.font.pixelSize * 1.6)
            let next = editor.positionToRectangle(position).y
            if (next <= y + 1) break
            result.push({y: next, position: position})
            y = next
        }
        pages = result
        locate()
    }
    function locate() {
        let y = editor.positionToRectangle(reader.position).y
        let found = 0
        for (let i = 0; i < pages.length; i++) {
            if (pages[i].y <= y + 0.5) found = i
        }
        page = found
        editor.select(reader.position, reader.end_position)
    }
    function turn(delta) {
        if (playback) return
        let next = Math.max(0, Math.min(pages.length - 1, page + delta))
        if (next === page) return
        page = next
        reader.select(pages[next].position)
        editor.select(reader.position, reader.end_position)
    }
    function selectStart(position) {
        if (!ready || playback) return
        reader.select(position)
        editor.select(reader.position, reader.end_position)
        editor.forceActiveFocus()
    }
    function begin() {
        if (!playback) {
            reader.select(editor.selectionStart === editor.selectionEnd ? editor.cursorPosition : editor.selectionStart)
            playback = true
        }
        reader.play()
    }
    function returnToText() {
        reader.pause()
        playback = false
        locate()
        editor.forceActiveFocus()
    }
    onClosing: reader.persist()
    Component.onCompleted: {
        reader.initialize()
        ready = true
        reflow.restart()
        if (smokeMode !== "") smokeTimer.start()
    }
    Timer { id: reflow; interval: 80; onTriggered: window.paginate() }
    Timer {
        id: playbackTimer
        interval: Math.max(1, reader.delay_ms)
        running: reader.playing
        repeat: true
        onTriggered: reader.tick()
    }
    Shortcut { sequence: "Right"; enabled: window.shortcutsActive && !playback && !speedBox.activeFocus; onActivated: turn(1) }
    Shortcut { sequence: "Left"; enabled: window.shortcutsActive && !playback && !speedBox.activeFocus; onActivated: turn(-1) }
    Shortcut { sequence: "Space"; enabled: window.shortcutsActive && !speedBox.activeFocus; onActivated: reader.playing ? reader.pause() : begin() }
    Shortcut { sequence: "Escape"; enabled: window.shortcutsActive; onActivated: returnToText() }
    FileDialog {
        id: openDialog
        title: "Open normalized book JSON"
        nameFilters: ["Book JSON (*.json)"]
        onAccepted: {
            reader.open_book(support.localPath(selectedFile))
            playback = false
            reflow.restart()
        }
    }
    Dialog {
        id: recoveryDialog
        title: "Confirm recovered location"
        anchors.centerIn: parent
        width: Math.min(window.width - 80, 600)
        modal: true
        standardButtons: Dialog.Ok | Dialog.Cancel
        contentItem: Label {
            textFormat: Text.PlainText
            text: reader.recovery + "\n\nUse this location? Cancel keeps the original saved position."
            wrapMode: Text.Wrap
        }
        onAccepted: { reader.confirm_recovery(); playback = false; window.locate() }
    }
    Dialog {
        id: chooseLocationDialog
        title: "Replace the old saved location?"
        anchors.centerIn: parent
        width: Math.min(window.width - 80, 600)
        modal: true
        standardButtons: Dialog.Ok | Dialog.Cancel
        contentItem: Label {
            textFormat: Text.PlainText
            text: "Save the selected passage in this edition: " + reader.word + "\n\nThe old edition's saved location will be replaced only if you confirm."
            wrapMode: Text.Wrap
        }
        onAccepted: { reader.confirm_selection(); window.locate() }
    }
    property alias noteVisible: noteDialog.visible
    property alias noteText: noteLabel.text
    function showNote(link) {
        if (link.startsWith("note:")) {
            noteLabel.text = reader.note_text(link.slice(5))
            noteDialog.open()
        }
    }
    function closeNote() { noteDialog.close() }
    Dialog {
        id: noteDialog
        title: "Note"
        anchors.centerIn: parent
        width: Math.min(window.width - 80, 600)
        modal: true
        standardButtons: Dialog.Close
        contentItem: Label { id: noteLabel; textFormat: Text.PlainText; wrapMode: Text.Wrap }
    }
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 12
        anchors.topMargin: Math.max(42, Math.round(window.height * 0.05))
        spacing: 6
        RowLayout {
            visible: controlsVisible && !playback
            Layout.fillWidth: true
            Label { text: reader.title; textFormat: Text.PlainText; elide: Text.ElideRight; Layout.fillWidth: true; font.pixelSize: 15 }
            Button { text: "Open JSON"; onClicked: openDialog.open() }
            ToolButton { text: "A−"; Accessible.name: "Decrease reading font"; onClicked: { preserveSelection(); textSize = Math.max(14, textSize - 2); reflow.restart() } }
            ToolButton { text: "A+"; Accessible.name: "Increase reading font"; onClicked: { preserveSelection(); textSize = Math.min(38, textSize + 2); reflow.restart() } }
        }
        Item {
            id: stage
            Layout.fillWidth: true
            Layout.fillHeight: true
            Item {
                id: viewport
                width: Math.min(readingMetrics.averageCharacterWidth * 65, Math.max(360, window.width - readingMetrics.averageCharacterWidth * 20), Math.max(0, parent.width - 24))
                height: parent.height
                anchors.horizontalCenter: parent.horizontalCenter
                visible: !playback
                onWidthChanged: { preserveSelection(); reflow.restart() }
                onHeightChanged: { preserveSelection(); reflow.restart() }
                Item {
                    width: parent.width
                    height: page + 1 < pages.length ? Math.min(viewport.height, pages[page + 1].y - pageY) : viewport.height
                    clip: true
                    TextEdit {
                        id: editor
                        objectName: "readingText"
                        y: -pageY
                        width: parent.width
                        height: contentHeight
                        text: reader.html
                        textFormat: TextEdit.RichText
                        onLinkActivated: link => window.showNote(link)
                        readOnly: true
                        selectByMouse: true
                        persistentSelection: true
                        wrapMode: TextEdit.Wrap
                        font.family: "IBM Plex Mono"
                        font.pixelSize: window.textSize * theme.textScale
                        color: theme.foreground
                        selectionColor: theme.selection
                        selectedTextColor: theme.selectionForeground
                        renderType: theme.textScale % 1 === 0 ? Text.NativeRendering : Text.QtRendering
                        onContentHeightChanged: reflow.restart()
                        TapHandler {
                            acceptedButtons: Qt.LeftButton
                            gesturePolicy: TapHandler.DragThreshold
                            onTapped: (eventPoint, button) => {
                                const point = eventPoint.position
                                if (editor.linkAt(point.x, point.y) !== "") return
                                const position = editor.positionAt(point.x, point.y)
                                Qt.callLater(() => window.selectStart(position))
                            }
                        }
                        Accessible.name: "Book text. Click a word to highlight the starting point, then press Space to play."
                    }
                }
            }
            Column {
                visible: playback
                anchors.centerIn: parent
                width: parent.width
                spacing: 18
                Text {
                    text: reader.word
                    textFormat: Text.PlainText
                    color: theme.foreground
                    font.family: "IBM Plex Mono"
                    font.pixelSize: Math.min(54, stage.width / Math.max(10, text.length) * 1.5)
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.Wrap
                    Accessible.role: Accessible.StaticText
                    Accessible.name: text
                }
                Shape {
                    objectName: "playbackShelf"
                    width: 42 * theme.textScale
                    height: 10 * theme.textScale
                    anchors.horizontalCenter: parent.horizontalCenter
                    ShapePath {
                        strokeColor: theme.accent
                        strokeWidth: 3.5 * theme.textScale
                        fillColor: "transparent"
                        capStyle: ShapePath.RoundCap
                        joinStyle: ShapePath.RoundJoin
                        startX: 2 * theme.textScale
                        startY: 2 * theme.textScale
                        PathQuad {
                            x: 8 * theme.textScale; y: 8 * theme.textScale
                            controlX: 2 * theme.textScale; controlY: 8 * theme.textScale
                        }
                        PathLine { x: 34 * theme.textScale; y: 8 * theme.textScale }
                        PathQuad {
                            x: 40 * theme.textScale; y: 2 * theme.textScale
                            controlX: 40 * theme.textScale; controlY: 8 * theme.textScale
                        }
                    }
                }
            }
        }
        Button {
            visible: reader.recovery.length > 0
            text: "Review recovered location"
            onClicked: recoveryDialog.open()
        }
        Button {
            visible: reader.recovery_needed
            text: "Use selected location for this edition"
            onClicked: { reader.select(editor.selectionStart); chooseLocationDialog.open() }
        }
        Label {
            Layout.fillWidth: true
            visible: reader.status !== "Stopped" && reader.status !== "Reading"
            text: reader.status
            textFormat: Text.PlainText
            font.pixelSize: 12
            opacity: 0.7
            wrapMode: Text.Wrap
            Accessible.name: "Reader status: " + text
        }
        RowLayout {
            Layout.fillWidth: true
            opacity: 0.65
            ToolButton { text: "⋯"; Accessible.name: "Reader controls"; onClicked: controlsVisible = !controlsVisible }
            ToolButton { text: playback ? "Return to text" : "Previous"; onClicked: playback ? returnToText() : turn(-1) }
            Label { visible: !playback; text: (page + 1) + " / " + pages.length; opacity: 0.6 }
            ToolButton { visible: !playback; text: "Next"; onClicked: turn(1) }
            Item { Layout.fillWidth: true }
            Label { visible: controlsVisible; text: "WPM"; opacity: 0.6 }
            SpinBox { id: speedBox; visible: controlsVisible; from: 60; to: 1200; stepSize: 25; value: reader.speed; onValueModified: reader.change_speed(value); Accessible.name: "Words per minute" }
            ToolButton { text: reader.playing ? "Pause" : playback ? "Continue" : "Play selection"; onClicked: reader.playing ? reader.pause() : begin() }
        }
    }
    Timer { id: smokeTimer; interval: 700; onTriggered: smoke.run() }
    Smoke {
        id: smoke
        host: window
        readerObject: reader
        textEditor: editor
        pageViewport: viewport
    }
}
