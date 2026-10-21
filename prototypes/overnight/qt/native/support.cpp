#include "support.h"
#include <QCoreApplication>
#include <QGuiApplication>
#include <QQuickWindow>
#include <QQuickItem>
#include <QMouseEvent>
#include <QKeyEvent>
#include <QQuickTextDocument>
#include <QTextDocument>
#include <QTextBlock>
#include <QTextFragment>
#include <QImage>
#include <QTextImageFormat>
#include <QTextCursor>
#include <algorithm>
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
    int italic = 0, bold = 0, images = 0, loadedImages = 0, headings = 0;
    QVariantList imageSizes;
    for (auto block = document->begin(); block.isValid(); block = block.next()) {
        if (block.blockFormat().headingLevel() > 0) ++headings;
        for (auto iterator = block.begin(); !iterator.atEnd(); ++iterator) {
            auto fragment = iterator.fragment();
            auto format = fragment.charFormat();
            if (format.fontItalic()) ++italic;
            if (format.fontWeight() >= QFont::Bold) ++bold;
            if (format.isImageFormat()) {
                ++images;
                auto resource = document->resource(QTextDocument::ImageResource, QUrl(format.toImageFormat().name()));
                auto image = qvariant_cast<QImage>(resource);
                if (!image.isNull()) {
                    ++loadedImages;
                    auto size = format.toImageFormat();
                    imageSizes.append(QVariantMap{{"width", size.width()}, {"height", size.height()},
                        {"naturalWidth", image.width()}, {"naturalHeight", image.height()}});
                }
            }
        }
    }
    return {{"plainText", document->toPlainText()}, {"italic", italic}, {"bold", bold},
            {"imageSizes", imageSizes}, {"images", images}, {"loadedImages", loadedImages}, {"headings", headings}};
}

void Support::fitImages(QObject *editor, double width, double height) {
    auto *quick = qvariant_cast<QQuickTextDocument *>(editor->property("textDocument"));
    if (!quick || !quick->textDocument() || width <= 0 || height <= 0) return;
    auto *document = quick->textDocument();
    QList<QPair<int, QTextImageFormat>> updates;
    for (auto block = document->begin(); block.isValid(); block = block.next()) {
        for (auto it = block.begin(); !it.atEnd(); ++it) {
            auto fragment = it.fragment();
            if (!fragment.charFormat().isImageFormat()) continue;
            auto format = fragment.charFormat().toImageFormat();
            auto image = qvariant_cast<QImage>(document->resource(QTextDocument::ImageResource, QUrl(format.name())));
            if (image.isNull()) continue;
            double scale = std::min({1.0, width / image.width(), height / image.height()});
            double w = image.width() * scale, h = image.height() * scale;
            if (qAbs(format.width() - w) < 0.01 && qAbs(format.height() - h) < 0.01) continue;
            format.setWidth(w);
            format.setHeight(h);
            updates.append({fragment.position(), format});
        }
    }
    for (const auto &update : updates) {
        QTextCursor cursor(document);
        cursor.setPosition(update.first);
        cursor.movePosition(QTextCursor::NextCharacter, QTextCursor::KeepAnchor);
        cursor.setCharFormat(update.second);
    }
}

bool Support::click(QObject *object, double x, double y) {
    auto *item = qobject_cast<QQuickItem *>(object);
    if (!item || !item->window()) return false;
    auto *window = item->window();
    const auto local = item->mapToScene(QPointF(x, y));
    const QPointF global = window->mapToGlobal(local.toPoint());
    QMouseEvent press(QEvent::MouseButtonPress, local, global, Qt::LeftButton, Qt::LeftButton, Qt::NoModifier);
    QMouseEvent release(QEvent::MouseButtonRelease, local, global, Qt::LeftButton, Qt::NoButton, Qt::NoModifier);
    QCoreApplication::sendEvent(window, &press);
    QCoreApplication::sendEvent(window, &release);
    return true;
}
void Support::pressSpace() {
    auto *window = QGuiApplication::focusWindow();
    if (!window) return;
    QKeyEvent press(QEvent::KeyPress, Qt::Key_Space, Qt::NoModifier, " ");
    QKeyEvent release(QEvent::KeyRelease, Qt::Key_Space, Qt::NoModifier, " ");
    QCoreApplication::sendEvent(window, &press);
    QCoreApplication::sendEvent(window, &release);
}
