#pragma once
#include <QObject>
#include <QUrl>
#include <QVariantMap>
class Support : public QObject {
    Q_OBJECT
public:
    using QObject::QObject;
    Q_INVOKABLE void finish(int code);
    Q_INVOKABLE bool capture(const QString &path);
    Q_INVOKABLE QString localPath(const QUrl &url) { return url.toLocalFile(); }
    Q_INVOKABLE void fitImages(QObject *editor, double width, double height);
    Q_INVOKABLE QVariantMap inspectDocument(QObject *editor);
};
