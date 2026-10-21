#pragma once
#include <QObject>
#include <QUrl>
#include <QVariantMap>
#include <QHash>
#include <QImage>
#include <QSet>
#include <QThreadPool>
#include <QPointer>
#include <QTextDocument>
class Support : public QObject {
    Q_OBJECT
public:
    using QObject::QObject;
    Q_INVOKABLE void installImages(QObject *editor);
    Q_INVOKABLE QString imageHtml(const QString &html, const QString &sources);
    Q_SIGNAL void imagesReady();
    Q_INVOKABLE bool memoryReport(bool finished, int work);
    Q_INVOKABLE void finish(int code);
    Q_INVOKABLE bool click(QObject *item, double x, double y);
    Q_INVOKABLE void pressSpace();
    Q_INVOKABLE bool capture(const QString &path);
    Q_INVOKABLE QString localPath(const QUrl &url) { return url.toLocalFile(); }
    Q_INVOKABLE void fitImages(QObject *editor, double width, double height);
    Q_INVOKABLE QVariantMap inspectDocument(QObject *editor);
private:
    QHash<QString, QImage> images;
    QSet<QString> wanted;
    QThreadPool imagePool;
    QHash<QString, QString> pendingImages;
    bool imageLoading = false;
    QString loadingImage;
    QPointer<QTextDocument> imageDocument;
    void loadNextImage();
};
