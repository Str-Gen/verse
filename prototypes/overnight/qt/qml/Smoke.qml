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
    property var continuation: null
    function afterPreparation(callback) { continuation = callback; prepared.start() }
    property Timer prepared: Timer { interval: 25; repeat: true; onTriggered: { if (readerObject.preparing) return; stop(); exposed.start() } }
    property Timer exposed: Timer { interval: 30; onTriggered: { let callback = continuation; continuation = null; callback() } }
    function run() {
        let reader = readerObject
        let editor = textEditor
        if (smokeMode === "experience") {
            let panel = host.vocabularyView
            let selected = reader.plain_text.indexOf("crossed")
            editor.select(selected, selected + 7)
            reader.change_speed(60)
            host.begin()
            check(reader.preparing && reader.word === "crossed", "preparation holds the selected source word")
            reader.pause()
            check(!reader.preparing && !reader.playing, "pause cancels preparation")
            reader.play()
            check(reader.preparing, "cancelled preparation starts fresh")
            let empty = JSON.parse(reader.vocabulary(JSON.stringify({action:"capture",scope:"seconds"})))
            check(!!empty.error, "preparation does not enter capture history")
            afterPreparation(() => {
                check(!reader.preparing && reader.word === "crossed", "pulse finishes before the first reading interval")
                reader.pause()
                reader.play()
                check(!reader.preparing && reader.delay_ms === 1000, "brief pause resumes immediately at full selected speed")
                reader.pause()
                panel.capture()
                panel.showReview()
                check(panel.visible && panel.review.captures.length === 1, "native capture reaches book-specific review")
                let capture = panel.review.captures[0]
                check(capture.context === "crossed" && capture.status === "awaiting_key", "offline capture saves only actually displayed text")
                panel.request({action:"feedback",capture:capture.id,candidate:0,choice:"recognition"})
                panel.refresh()
                check(panel.review.words.length === 1, "native feedback persists a saved word")
                let definition = panel.request({action:"definition",word:"crossed"})
                check(definition.available && definition.entries[0].senses.length === 2, "native local lookup retains all dictionary senses")
                panel.practicing = true
                panel.practiceAt = 0
                panel.card()
                check(panel.practiceWord === "crossed", "practice flashes the saved word")
                if (screenshotPath !== "") check(support.capture(screenshotPath), "native vocabulary screenshot saved")
                panel.close()
                console.log("EXPERIENCE COMPLETE")
                support.finish(0)
            })
            return
        }
        if (smokeMode === "notes") {
            reader.set_options(true, false)
            let marker = reader.plain_text.indexOf("A 🦊")
            editor.select(marker, marker + 1)
            reader.change_speed(600)
            host.begin()
            afterPreparation(() => {
            check(reader.word === "Note," && Math.abs(reader.delay_ms - 250) <= 2, "optional note uses punctuation pacing")
            reader.tick()
            check(reader.word === "done." && reader.position === marker, "note keeps the main reference anchor")
            reader.tick()
            check(reader.word === "🦊" && reader.playing, "note automatically returns after its reference")
            reader.pause()
            reader.select(marker)
            reader.play()
            host.returnToText()
            check(reader.word === "A" && !reader.playing && editor.selectionStart === marker, "return to text leaves the note at its reference")
            console.log("NOTES COMPLETE")
            support.finish(0)
            })
            return
        }
        if (smokeMode === "parity") {
            if (phase === 0) {
                check(support.click(host.controlsButton, host.controlsButton.width / 2, host.controlsButton.height / 2), "native click on three dots delivered")
                check(host.controlsVisible, "three-dot action opens contents and controls")
                check(JSON.parse(reader.chapters).length === 2, "sidebar exposes book chapters")
                phase = 1
                later.start()
            } else if (phase === 1) {
                let item = host.chaptersView.itemAtIndex(1)
                check(item !== null && support.click(item, item.width / 2, item.height / 2), "native chapter row click delivered")
                check(reader.chapter_index === 1 && !reader.playing && !host.playback, "chapter jump is stopped in reading mode")
                phase = 2
                later.start()
            } else {
                check(editor.selectionStart === reader.position, "chapter jump highlights its source")
                check(Math.abs(editor.positionToRectangle(reader.position).y - host.pageY) < 1, "chapter opens at the top of its page")
                check(pageViewport.width > 0 && pageViewport.width < host.width, "reading viewport fits beside sidebar")
                if (screenshotPath !== "") check(support.capture(screenshotPath), "sidebar screenshot saved")
                host.toggleControls()
                check(!host.controlsVisible, "three-dot action hides sidebar")
                let end = reader.plain_text.indexOf("inland.")
                editor.select(end, end + 7)
                host.begin()
                afterPreparation(() => {
                reader.tick()
                check(host.feedbackVisible && !reader.playing, "native chapter boundary opens optional feedback")
                host.continueChapter(true)
                reader.pause()
                check(!host.feedbackVisible && reader.history.indexOf("Comfortable") >= 0, "feedback saves and continues to next chapter")
                reader.set_options(true, false)
                check(reader.follow_notes && !reader.feedback_enabled, "optional playback settings reach Rust")
                console.log("PARITY COMPLETE")
                support.finish(0)
                })
            }
            return
        }
        if (smokeMode === "compound") {
            check(reader.plain_text === "state-of-the-art.", "cached compound keeps its conventional text")
            reader.change_speed(600)
            editor.cursorPosition = 0
            host.begin()
            afterPreparation(() => {
            const beats = ["state-", "of-", "the-", "art."]
            const delays = [80, 80, 80, 290]
            for (let i = 0; i < beats.length; i++) {
                check(reader.word === beats[i], "compound beat " + beats[i])
                check(Math.abs(reader.delay_ms - delays[i] * 2) <= 3, "compound cadence " + delays[i] + " ms")
                if (i + 1 < beats.length) reader.tick()
            }
            reader.pause()
            host.returnToText()
            check(editor.selectedText === "art." && reader.position === 13, "return highlights the exact last compound beat")
            console.log("COMPOUND COMPLETE")
            support.finish(0)
            })
            return
        }
        if (smokeMode === "selection") {
            if (phase === 0) {
                anchor = reader.plain_text.indexOf("crossed")
                check(anchor >= 0, "click target exists")
                let rect = editor.positionToRectangle(anchor + 2)
                check(support.click(editor, rect.x + 1, rect.y + rect.height / 2), "native pointer click delivered")
                phase = 1
                later.start()
            } else if (phase === 1) {
                check(editor.selectedText === "crossed", "single click visibly highlights the whole word")
                check(reader.word === "crossed" && reader.position === anchor, "highlight matches playback starting point")
                host.contentItem.forceActiveFocus()
                check(editor.persistentSelection && editor.selectedText === "crossed", "highlight remains when focus moves away")
                if (screenshotPath !== "") check(support.capture(screenshotPath), "clicked-word screenshot saved")
                support.pressSpace()
                check(host.playback && reader.playing && reader.word === "crossed", "Space starts from the visibly selected word")
                reader.pause()
                phase = 2
                later.start()
            } else {
                if (screenshotPath !== "") check(support.capture(screenshotPath.replace(".png", "-playback.png")), "shelf screenshot saved")
                host.returnToText()
                check(editor.selectedText === "crossed", "return restores visible starting word")
                console.log("SELECTION COMPLETE")
                support.finish(0)
            }
            return
        }
        if (smokeMode === "independent") {
            let text = support.inspectDocument(editor).plainText
            check(text === reader.plain_text, "independent export agrees with Qt native layout")
            check(text.indexOf("unbelievable") >= 0 && text.indexOf("A😀B") >= 0 && text.indexOf("𝄞tone") >= 0, "independent Unicode words remain whole")
            check(text.indexOf("Hidden") < 0 && text.indexOf("hidden inline") < 0 && text.indexOf("Publisher script") < 0, "independent hidden content absent in native renderer")
            let start = text.indexOf("A😀B")
            editor.select(start + 1, start + 3)
            host.begin()
            check(reader.word === "A😀B" && reader.position === start, "native UTF-16 selection resolves scalar source word")
            afterPreparation(() => {
            reader.tick()
            reader.pause()
            check(reader.word === "𝄞tone", "next whole astral word displayed")
            let last = reader.position
            host.returnToText()
            check(editor.selectionStart === last, "independent source round trip through native layout")
            host.showNote("note:text.xhtml#n1")
            check(host.noteVisible && host.noteText === "A small note.", "independent detached note retained in manual dialog")
            host.closeNote()
            if (screenshotPath !== "") check(support.capture(screenshotPath), "independent native screenshot saved")
            console.log("INDEPENDENT COMPLETE")
            support.finish(0)
            })
            return
        }
        if (smokeMode === "core") {
            let report = support.inspectDocument(editor)
            check(report.plainText === reader.plain_text, "integrated core text matches native UTF-16 layout")
            check(report.italic > 0 && report.bold > 0 && report.headings > 0, "integrated core structure renders natively")
            check(report.images === 5 && report.loadedImages === 5, "all five core raster occurrences decode in Qt")
            for (let size of report.imageSizes) {
                check(Math.abs(size.width * size.naturalHeight - size.height * size.naturalWidth) < 0.1,
                      "image preserves intrinsic aspect ratio")
                check(size.width <= size.naturalWidth && size.height <= size.naturalHeight,
                      "small images are not enlarged")
            }
            check(report.plainText.indexOf("[]") < 0, "no empty image captions")
            support.fitImages(editor, 20, 10)
            for (let size of support.inspectDocument(editor).imageSizes) {
                check(size.width <= 20 && size.height <= 10 &&
                      Math.abs(size.width * size.naturalHeight - size.height * size.naturalWidth) < 0.1,
                      "images fit narrow bounds without distortion")
            }
            host.paginate()
            check(support.inspectDocument(editor).plainText === reader.plain_text, "image resize preserves source positions")
            host.showNote("note:EPUB/notes.xhtml#n1")
            check(host.noteVisible && host.noteText.indexOf("manual note") >= 0, "core note link opens manual note dialog")
            host.closeNote()
            let joined = report.plainText.indexOf("naïveté")
            check(joined >= 0, "whole multi-run word retained")
            editor.select(joined + 2, joined + 5)
            host.begin()
            check(reader.word === "naïveté" && reader.playing, "partial multi-run selection starts whole source word")
            reader.tick()
            reader.pause()
            let displayed = reader.position
            host.returnToText()
            check(editor.selectionStart === displayed, "integrated playback returns to last displayed source")
            let image = report.plainText.indexOf("\ufffc")
            reader.select(image)
            reader.play()
            check(!reader.playing && reader.status.indexOf("Inspect") >= 0, "core image pauses for inspection")
            host.returnToText()
            if (screenshotPath !== "") check(support.capture(screenshotPath), "integrated core screenshot saved")
            console.log("CORE COMPLETE")
            support.finish(0)
            return
        }
        if (smokeMode === "recover-pending" || smokeMode === "recover-confirm") {
            check(reader.recovery_needed && reader.recovery.length > 0 && !reader.playing, "changed edition offers recovery while stopped")
            if (smokeMode === "recover-pending") {
                reader.persist()
                check(reader.recovery_needed, "save cannot silently accept recovery")
                console.log("RECOVERY_PENDING COMPLETE")
            } else {
                reader.confirm_recovery()
                check(!reader.recovery_needed && reader.position === 0 && !reader.playing, "confirmed recovery adopts chapter start while stopped")
                console.log("RECOVERY_CONFIRM COMPLETE")
            }
            support.finish(0)
            return
        }
        if (smokeMode === "resume" || smokeMode === "resume-chapter") {
            check(!reader.playing && !host.playback, "restart is stopped in conventional view")
            check(reader.speed === 425 && (smokeMode === "resume-chapter" ? reader.position === 0 : reader.word === "crossed"), "per-book speed and source restored in new process")
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
            afterPreparation(() => {
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
            })
        } else if (phase === 1) {
            check(reader.position === anchor && reader.word === anchorWord, "font and resize retain source passage")
            let rectangle = editor.positionToRectangle(anchor)
            check(rectangle.y >= host.pageY && rectangle.y < host.pageY + pageViewport.height, "retained passage visible after reflow")
            reader.persist()
            if (screenshotPath !== "") check(support.capture(screenshotPath), "synthetic screenshot saved")
            playbackStart = reader.position
            host.begin()
            phase = 2
            later.interval = 2500
            later.start()
        } else if (phase === 2) {
            later.interval = 300
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
