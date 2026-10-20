#include "support.h"
#include <QCoreApplication>
#include <QGuiApplication>
#include <QQuickWindow>
#include <QQuickTextDocument>
#include <QTextDocument>
#include <QTextBlock>
#include <QTextFragment>
#include <QImage>
void Support::finish(int code) { QCoreApplication::exit(code); }
bool Support::capture(const QString &path) {
    for (auto *window : QGuiApplication::allWindows()) {
        if (auto *quick = qobject_cast<QQuickWindow *>(window)) {
            if (quick->isVisible()) return quick->grabWindow().save(path);
        }
    }
    return false;
}
QVariantMap Support::inspectDocument(QObject *editor) {
    auto *quick = qvariant_cast<QQuickTextDocument *>(editor->property("textDocument"));
    if (!quick || !quick->textDocument()) return {};
    auto *document = quick->textDocument();
    int italic = 0, bold = 0, images = 0, headings = 0;
    for (auto block = document->begin(); block.isValid(); block = block.next()) {
        if (block.blockFormat().headingLevel() > 0) ++headings;
        for (auto iterator = block.begin(); !iterator.atEnd(); ++iterator) {
            auto fragment = iterator.fragment();
            auto format = fragment.charFormat();
            if (format.fontItalic()) ++italic;
            if (format.fontWeight() >= QFont::Bold) ++bold;
            if (format.isImageFormat()) ++images;
        }
    }
    return {{"plainText", document->toPlainText()}, {"italic", italic}, {"bold", bold},
            {"images", images}, {"headings", headings}};
}
