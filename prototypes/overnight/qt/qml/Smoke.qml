import QtQuick
QtObject {
    property var host
    property var readerObject
    property var textEditor
    property var pageViewport
    property int phase: 0
    property int anchor: 0
    property string anchorWord: ""
    property int playbackStart: 0
    property int displayedBeforeReturn: 0
    function check(condition, label) {
        if (!condition) {
            console.error("SMOKE FAIL: " + label)
            support.finish(1)
            throw new Error(label)
        }
        console.log("SMOKE PASS: " + label)
    }
    function run() {
        let reader = readerObject
        let editor = textEditor
        if (smokeMode === "recover-pending" || smokeMode === "recover-confirm") {
            check(reader.recovery_needed && reader.recovery.length > 0 && !reader.playing, "changed edition offers recovery while stopped")
            if (smokeMode === "recover-pending") {
                reader.persist()
                check(reader.recovery_needed, "save cannot silently accept recovery")
                console.log("RECOVERY_PENDING COMPLETE")
            } else {
                reader.confirm_recovery()
                check(!reader.recovery_needed && reader.word === "crossed" && !reader.playing, "confirmed recovery adopts exact passage while stopped")
                console.log("RECOVERY_CONFIRM COMPLETE")
            }
            support.finish(0)
            return
        }
        if (smokeMode === "resume") {
            check(!reader.playing && !host.playback, "restart is stopped in conventional view")
            check(reader.speed === 425 && reader.word === "crossed", "per-book speed and source restored in new process")
            check(editor.selectionStart === reader.position, "restored source highlighted")
            console.log("RESUME COMPLETE")
            support.finish(0)
            return
        }
        if (phase === 0) {
            let report = support.inspectDocument(editor)
            check(report.plainText === reader.plain_text, "Qt plain text matches Rust UTF-16 mapping")
            check(report.italic > 0 && report.bold > 0 && report.headings > 0 && report.images > 0, "native heading/emphasis/image formats")
            check(report.plainText.indexOf("• ") >= 0 && report.plainText.indexOf("Note: ") >= 0, "list and note preserved")
            check(host.pages.length > 2, "real Qt layout produces multiple pages")
            let allLinesFit = true
            let pageIndex = 0
            for (let offset = 0; offset < editor.length; offset++) {
                let rect = editor.positionToRectangle(offset)
                while (pageIndex + 1 < host.pages.length && rect.y >= host.pages[pageIndex + 1].y - 0.5) pageIndex++
                let bottom = pageIndex + 1 < host.pages.length ? host.pages[pageIndex + 1].y : host.pages[pageIndex].y + pageViewport.height
                if (rect.y + rect.height > bottom + 1) allLinesFit = false
            }
            check(allLinesFit, "every rendered character line fits one page without clipping")
            let initial = reader.position
            host.turn(1)
            check(reader.position > initial && host.page === 1, "page navigation saves first source passage")
            host.turn(-1)
            check(host.page === 0, "previous page")
            let fox = report.plainText.indexOf("🦊")
            check(fox >= 0, "non-BMP fixture present")
            editor.select(fox, fox + 2)
            host.begin()
            check(reader.word === "🦊" && reader.position === fox, "selection starts exact Unicode source piece")
            reader.tick()
            reader.pause()
            check(reader.word === "crossed", "playback advances")
            anchor = reader.position
            anchorWord = reader.word
            host.returnToText()
            check(editor.selectionStart === anchor, "return highlights last displayed passage")
            let savedTitle = reader.title
            let savedHtml = reader.html
            reader.open_book("/nonexistent/overnight-qt-reader.json")
            check(reader.title === savedTitle && reader.html === savedHtml && reader.position === anchor, "invalid book open retains current book and source")
            reader.change_speed(425)
            host.textSize = 28
            host.width = 720
            host.height = 650
            host.paginate()
            phase = 1
            later.start()
        } else if (phase === 1) {
            check(reader.position === anchor && reader.word === anchorWord, "font and resize retain source passage")
            let rectangle = editor.positionToRectangle(anchor)
            check(rectangle.y >= host.pageY && rectangle.y < host.pageY + pageViewport.height, "retained passage visible after reflow")
            reader.persist()
            if (screenshotPath !== "") check(support.capture(screenshotPath), "synthetic screenshot saved")
            playbackStart = reader.position
            host.begin()
            phase = 2
            later.start()
        } else if (phase === 2) {
            check(reader.position > playbackStart, "real QML timer advances visible playback")
            reader.pause()
            displayedBeforeReturn = reader.position
            if (screenshotPath !== "") check(support.capture(screenshotPath.replace(".png", "-playback.png")), "playback screenshot saved")
            host.returnToText()
            phase = 3
            later.start()
        } else if (phase === 3) {
            check(reader.position === displayedBeforeReturn && editor.selectionStart === displayedBeforeReturn, "return after visible playback preserves last displayed source")
            let imagePosition = reader.plain_text.indexOf("\ufffc")
            check(imagePosition >= 0, "image has a native document position")
            reader.select(imagePosition)
            host.locate()
            phase = 4
            later.start()
        } else if (phase === 4) {
            if (screenshotPath !== "") check(support.capture(screenshotPath.replace(".png", "-content.png")), "image and structured content screenshot saved")
            reader.select(playbackStart)
            host.locate()
            reader.persist()
            console.log("SMOKE COMPLETE")
            support.finish(0)
        }
    }
    property Timer later: Timer { interval: 300; onTriggered: run() }
}
