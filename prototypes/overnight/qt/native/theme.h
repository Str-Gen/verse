#pragma once

#include <QColor>
#include <QFileSystemWatcher>
#include <QObject>
#include <QTimer>

class QDBusVariant;

// Platform theme adapter; reader state and behavior remain in Rust.
class Theme final : public QObject {
    Q_OBJECT
    Q_PROPERTY(QColor background READ background NOTIFY changed)
    Q_PROPERTY(QColor foreground READ foreground NOTIFY changed)
    Q_PROPERTY(QColor accent READ accent NOTIFY changed)
    Q_PROPERTY(QColor selection READ selection NOTIFY changed)
    Q_PROPERTY(QColor selectionForeground READ selectionForeground NOTIFY changed)
    Q_PROPERTY(qreal textScale READ textScale NOTIFY changed)
public:
    explicit Theme(QObject *parent = nullptr);
    QColor background() const { return m_background; }
    QColor foreground() const { return m_foreground; }
    QColor accent() const { return m_accent; }
    QColor selection() const { return m_selection; }
    QColor selectionForeground() const { return m_selectionForeground; }
    qreal textScale() const { return m_textScale; }
    Q_INVOKABLE void refresh();
signals:
    void changed();
private slots:
    void portalChanged(const QString &nameSpace, const QString &key, const QDBusVariant &value);
private:
    void reloadColors();
    void rearmWatch();
    QString m_path;
    QFileSystemWatcher m_watcher;
    QTimer m_reload;
    QColor m_background, m_foreground, m_accent, m_selection, m_selectionForeground;
    qreal m_textScale = 1.0;
    bool m_systemDark = true;
};
