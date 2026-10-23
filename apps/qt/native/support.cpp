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
#include <QCryptographicHash>
#include <QRegularExpression>
#include <QFutureWatcher>
#include <QtConcurrentRun>
#include <QImageReader>
#include <QBuffer>
#include <QPointer>
#include <QJsonDocument>
#include <QJsonArray>
#include <QJsonObject>
#include <QSaveFile>
#include <QDir>
namespace {
QString imageKey(const QString &hash) {
    static const auto transparent = []() {
        QImage pixel(1, 1, QImage::Format_ARGB32_Premultiplied);
        pixel.fill(Qt::transparent);
        QByteArray bytes;
        QBuffer buffer(&bytes);
        buffer.open(QIODevice::WriteOnly);
        pixel.save(&buffer, "PNG");
        return QString::fromLatin1(bytes.toBase64());
    }();
    return QStringLiteral("data:image/png;name=") + hash + QStringLiteral(";base64,") + transparent;
}
}
void Support::finish(int code) { QCoreApplication::exit(code); }
bool Support::capture(const QString &path) {
    for (auto *window : QGuiApplication::allWindows()) {
        if (auto *quick = qobject_cast<QQuickWindow *>(window)) {
            if (quick->isVisible()) return quick->grabWindow().save(path);
        }
    }
    return false;
}
int Support::visibleInk(QObject *object) {
    auto *item = qobject_cast<QQuickItem *>(object);
    if (!item || !item->window()) return 0;
    auto *window = item->window();
    const auto frame = window->grabWindow();
    const auto scale = frame.devicePixelRatio();
    const auto bounds = item->mapRectToScene(item->boundingRect());
    const auto rect = QRectF(bounds.topLeft() * scale, bounds.size() * scale).toAlignedRect().intersected(frame.rect());
    const auto background = window->color();
    int ink = 0;
    for (int y = rect.top(); y <= rect.bottom(); ++y) {
        for (int x = rect.left(); x <= rect.right(); ++x) {
            const auto color = frame.pixelColor(x, y);
            if (qAbs(color.red() - background.red()) + qAbs(color.green() - background.green()) + qAbs(color.blue() - background.blue()) > 60) ++ink;
        }
    }
    return ink;
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
                if (format.toImageFormat().name().startsWith(QStringLiteral("data:image/png;name=")) &&
                    (!this->images.contains(format.toImageFormat().name()) || this->images.value(format.toImageFormat().name()).isNull())) image = QImage();
                if (!image.isNull()) {
                    ++loadedImages;
                    auto size = format.toImageFormat();
                    imageSizes.append(QVariantMap{{"position", fragment.position()}, {"width", size.width()}, {"height", size.height()},
                        {"naturalWidth", image.width()}, {"naturalHeight", image.height()}});
                }
            }
        }
    }
    return {{"plainText", document->toPlainText()}, {"italic", italic}, {"bold", bold},
            {"imageSizes", imageSizes}, {"images", images}, {"loadedImages", loadedImages}, {"headings", headings}};
}

bool Support::fitImages(QObject *editor, double width, double height) {
    auto *quick = qvariant_cast<QQuickTextDocument *>(editor->property("textDocument"));
    if (!quick || !quick->textDocument() || width <= 0 || height <= 0) return false;
    auto *document = quick->textDocument();
    QList<QPair<int, QTextImageFormat>> updates;
    for (auto block = document->begin(); block.isValid(); block = block.next()) {
        for (auto it = block.begin(); !it.atEnd(); ++it) {
            auto fragment = it.fragment();
            if (!fragment.charFormat().isImageFormat()) continue;
            auto format = fragment.charFormat().toImageFormat();
            auto image = qvariant_cast<QImage>(document->resource(QTextDocument::ImageResource, QUrl(format.name())));
            constexpr int intrinsicWidth = QTextFormat::UserProperty + 1;
            constexpr int intrinsicHeight = QTextFormat::UserProperty + 2;
            double naturalWidth = format.property(intrinsicWidth).toDouble();
            double naturalHeight = format.property(intrinsicHeight).toDouble();
            if (naturalWidth <= 0 || naturalHeight <= 0) {
                if ((format.width() <= 0 || format.height() <= 0) &&
                    format.name().startsWith(QStringLiteral("data:image/png;name=")) &&
                    !images.contains(format.name())) continue;
                naturalWidth = format.width() > 0 ? format.width() : image.width();
                naturalHeight = format.height() > 0 ? format.height() : image.height();
                if (naturalWidth <= 0 || naturalHeight <= 0) continue;
                format.setProperty(intrinsicWidth, naturalWidth);
                format.setProperty(intrinsicHeight, naturalHeight);
            }
            double scale = std::min({1.0, width / naturalWidth, height / naturalHeight});
            double w = naturalWidth * scale, h = naturalHeight * scale;
            if (qAbs(format.width() - w) < 0.01 && qAbs(format.height() - h) < 0.01 && fragment.charFormat().hasProperty(intrinsicWidth)) continue;
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
    return !updates.isEmpty();
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
void Support::pressSpace() { pressKey(Qt::Key_Space); }
void Support::pressKey(int key) {
    auto *window = QGuiApplication::focusWindow();
    if (!window) return;
    QKeyEvent press(QEvent::KeyPress, key, Qt::NoModifier);
    QKeyEvent release(QEvent::KeyRelease, key, Qt::NoModifier);
    QCoreApplication::sendEvent(window, &press);
    QCoreApplication::sendEvent(window, &release);
}

void Support::installImages(QObject *editor) {
    auto *quick = qvariant_cast<QQuickTextDocument *>(editor->property("textDocument"));
    if (!quick || !quick->textDocument()) return;
    auto *document = quick->textDocument();
    imageDocument = document;
    imagePool.setMaxThreadCount(1);
    document->setResourceProvider([this](const QUrl &url) -> QVariant {
        const auto found = images.constFind(url.toString());
        if (found != images.cend() && !found.value().isNull()) return found.value();
        QImage placeholder(1, 1, QImage::Format_ARGB32_Premultiplied);
        placeholder.fill(Qt::transparent);
        return placeholder;
    });
}

void Support::updateImage(const QString &key, const QImage &image) {
    if (!imageDocument) return;
    imageDocument->addResource(QTextDocument::ImageResource, QUrl(key), image);
    for (auto block = imageDocument->begin(); block.isValid(); block = block.next()) {
        for (auto it = block.begin(); !it.atEnd(); ++it) {
            const auto fragment = it.fragment();
            const auto format = fragment.charFormat();
            if (format.isImageFormat() && format.toImageFormat().name() == key)
                imageDocument->markContentsDirty(fragment.position(), fragment.length());
        }
    }
}

QString Support::imageHtml(const QString &html, const QString &resourceJson) {
    static const QRegularExpression expression(QStringLiteral("<img src=\"([^\"]+)\""));
    auto matches = expression.globalMatch(html);
    QString output;
    qsizetype offset = 0;
    QSet<QString> next;
    QList<QPair<QString, QString>> sources;
    const auto resources = QJsonDocument::fromJson(resourceJson.toUtf8()).array();
    for (const auto &resource : resources) {
        const auto entry = resource.toObject();
        const auto source = entry.value("source").toString();
        const auto key = imageKey(entry.value("key").toString());
        if (next.size() == 5) break;
        next.insert(key);
        if (!images.contains(key) && loadingImage != key) sources.append({key, source});
    }
    while (matches.hasNext()) {
        auto match = matches.next();
        auto source = match.captured(1);
        auto key = imageKey(source.startsWith(QStringLiteral("reader-image:")) ? source.mid(13) : QString::fromLatin1(
            QCryptographicHash::hash(source.toUtf8(), QCryptographicHash::Sha256).toHex()));
        output += html.mid(offset, match.capturedStart(1) - offset) + key;
        offset = match.capturedEnd(1);
        if (resources.isEmpty() && next.size() < 5 && !next.contains(key)) {
            next.insert(key);
            if (!images.contains(key) && loadingImage != key) sources.append({key, source});
        }
    }
    output += html.mid(offset);
    wanted = next;
    for (auto it = images.begin(); it != images.end();) {
        if (!wanted.contains(it.key())) {
            const auto key = it.key();
            it = images.erase(it);
            updateImage(key, QImage());
        }
        else ++it;
    }
    pendingImages.clear();
    for (const auto &source : sources) pendingImages.insert(source.first, source.second);
    loadNextImage();
    return output;
}

void Support::loadNextImage() {
    if (imageLoading || pendingImages.isEmpty()) return;
    auto source = pendingImages.begin();
    const auto key = source.key();
    const auto uri = source.value();
    pendingImages.erase(source);
    imageLoading = true;
    loadingImage = key;
        auto *watcher = new QFutureWatcher<QImage>(this);
        connect(watcher, &QFutureWatcher<QImage>::finished, this, [this, watcher, key]() {
            if (wanted.contains(key)) {
                images.insert(key, watcher->result());
                updateImage(key, watcher->result());
                emit imagesReady();
            }
            watcher->deleteLater();
            imageLoading = false;
            loadingImage.clear();
            loadNextImage();
        });
        watcher->setFuture(QtConcurrent::run(&imagePool, [uri]() {
            QByteArray bytes;
            QBuffer buffer(&bytes);
            QImageReader reader;
            if (uri.startsWith(QStringLiteral("data:"))) {
                bytes = QByteArray::fromBase64(uri.mid(uri.indexOf(',') + 1).toLatin1());
                buffer.open(QIODevice::ReadOnly);
                reader.setDevice(&buffer);
            } else {
                reader.setFileName(QUrl(uri).toLocalFile());
            }
            auto size = reader.size();
            if (size.width() > 1600 || size.height() > 1600)
                reader.setScaledSize(size.scaled(1600, 1600, Qt::KeepAspectRatio));
            return reader.read();
        }));
}

bool Support::memoryReport(bool finished, int work) {
    if (!qEnvironmentVariableIsSet("READER_MEMORY_TEST")) return false;
    const auto state = qEnvironmentVariable("READER_STATE_DIR");
    if (state.isEmpty()) return false;
    const auto directory = state + QStringLiteral("/self-test");
    if (!QDir().mkpath(directory)) return false;
    QSaveFile file(directory + (finished ? QStringLiteral("/self-test.json") : QStringLiteral("/memory-phase.json")));
    if (!file.open(QIODevice::WriteOnly)) return false;
    const QJsonObject report = finished ? QJsonObject{{"passed", true}, {"work", work}}
        : QJsonObject{{"phase", "reading"}};
    if (file.write(QJsonDocument(report).toJson(QJsonDocument::Compact)) < 0) return false;
    return file.commit();
}
