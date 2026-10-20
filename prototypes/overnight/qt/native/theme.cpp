// Adapted from Omawrite src/backend.cpp and src/systemtheme.cpp.
// Copyright (c) 2026 David Heinemeier Hansson, MIT. See repository LICENSE.
#include "theme.h"
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusReply>
#include <QDBusVariant>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QMap>
#include <QStyleHints>
#include <QTextStream>
#include <cmath>

namespace {
QVariant unwrap(QVariant value) {
    while (value.canConvert<QDBusVariant>()) value = value.value<QDBusVariant>().variant();
    return value;
}
QVariant readPortal(const QString &space, const QString &key) {
    auto bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) return {};
    auto request = QDBusMessage::createMethodCall("org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop", "org.freedesktop.portal.Settings", "Read");
    request << space << key;
    const QDBusReply<QDBusVariant> reply(bus.call(request, QDBus::Block, 150));
    return reply.isValid() ? unwrap(reply.value().variant()) : QVariant();
}
QString tomlValue(QString value) {
    value = value.trimmed();
    if (!value.isEmpty() && (value.front() == '"' || value.front() == '\'')) {
        const auto end = value.indexOf(value.front(), 1);
        if (end >= 0) return value.mid(1, end - 1);
    }
    return value;
}
qreal scaleValue(const QVariant &value) {
    bool ok = false;
    const auto scale = unwrap(value).toDouble(&ok);
    return ok && std::isfinite(scale) && scale > 0 ? qBound(0.5, scale, 3.0) : 1.0;
}
}

Theme::Theme(QObject *parent) : QObject(parent) {
    m_path = qEnvironmentVariable("READER_THEME_FILE");
    if (m_path.isEmpty())
        m_path = QDir::homePath() + "/.local/state/omarchy/current/theme/colors.toml";
    m_path = QFileInfo(m_path).absoluteFilePath();
    m_reload.setSingleShot(true);
    m_reload.setInterval(100);
    connect(&m_reload, &QTimer::timeout, this, [this] { reloadColors(); rearmWatch(); });
    connect(&m_watcher, &QFileSystemWatcher::fileChanged, &m_reload, qOverload<>(&QTimer::start));
    connect(&m_watcher, &QFileSystemWatcher::directoryChanged, &m_reload, qOverload<>(&QTimer::start));
    if (auto *hints = QGuiApplication::styleHints())
        connect(hints, &QStyleHints::colorSchemeChanged, this, &Theme::refresh);
    QDBusConnection::sessionBus().connect(QString(), "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Settings", "SettingChanged", this,
        SLOT(portalChanged(QString,QString,QDBusVariant)));
    refresh();
    rearmWatch();
}

void Theme::refresh() {
    const auto scheme = readPortal("org.freedesktop.appearance", "color-scheme").toUInt();
    if (scheme == 1 || scheme == 2) m_systemDark = scheme == 1;
    else {
        const auto *hints = QGuiApplication::styleHints();
        m_systemDark = !hints || hints->colorScheme() != Qt::ColorScheme::Light;
    }
    const auto scale = scaleValue(readPortal("org.gnome.desktop.interface", "text-scaling-factor"));
    const bool scaleChanged = !qFuzzyCompare(scale, m_textScale);
    m_textScale = scale;
    reloadColors();
    if (scaleChanged) emit changed();
}

void Theme::portalChanged(const QString &space, const QString &key, const QDBusVariant &value) {
    if (space == "org.gnome.desktop.interface" && key == "text-scaling-factor") {
        const auto scale = scaleValue(value.variant());
        if (!qFuzzyCompare(scale, m_textScale)) { m_textScale = scale; emit changed(); }
    } else if (key == "color-scheme" && space == "org.freedesktop.appearance") {
        const auto scheme = unwrap(value.variant()).toUInt();
        if (scheme == 1 || scheme == 2) { m_systemDark = scheme == 1; reloadColors(); }
        else refresh();
    } else if (key == "color-scheme" && space == "org.gnome.desktop.interface") {
        const auto scheme = unwrap(value.variant()).toString();
        if (scheme.contains("prefer-dark") || scheme.contains("prefer-light")) {
            m_systemDark = scheme.contains("prefer-dark");
            reloadColors();
        } else refresh();
    }
}

void Theme::reloadColors() {
    QMap<QString, QString> values;
    QFile file(m_path);
    if (file.open(QIODevice::ReadOnly | QIODevice::Text)) {
        QTextStream stream(&file);
        while (!stream.atEnd()) {
            const auto line = stream.readLine().trimmed();
            const auto equals = line.indexOf('=');
            if (!line.startsWith('#') && equals >= 0)
                values.insert(line.left(equals).trimmed(), tomlValue(line.mid(equals + 1)));
        }
    }
    bool dark = m_systemDark;
    const QColor page(values.value("background"));
    if (values.value("mode") == "dark") dark = true;
    else if (values.value("mode") == "light") dark = false;
    else if (page.isValid()) dark = 0.299 * page.redF() + 0.587 * page.greenF() + 0.114 * page.blueF() < 0.5;
    const auto color = [&values](const QString &key, QColor fallback) {
        const QColor parsed(values.value(key));
        return parsed.isValid() ? parsed : fallback;
    };
    const QColor background = color("background", QColor(dark ? "#101010" : "#ffffff"));
    const QColor foreground = color("foreground", QColor(dark ? "#eeeeee" : "#222324"));
    const QColor accent = color("accent", QColor(dark ? "#5584aa" : "#2077b2"));
    const QColor selection = color("selection", color("selection_background", QColor(dark ? "#186a9a" : "#2077b2")));
    const QColor selectedText = color("selection_foreground", color("bright_foreground", foreground));
    if (background == m_background && foreground == m_foreground && accent == m_accent &&
        selection == m_selection && selectedText == m_selectionForeground) return;
    m_background = background;
    m_foreground = foreground;
    m_accent = accent;
    m_selection = selection;
    m_selectionForeground = selectedText;
    emit changed();
}

void Theme::rearmWatch() {
    const auto watched = m_watcher.files() + m_watcher.directories();
    if (!watched.isEmpty()) m_watcher.removePaths(watched);
    if (QFile::exists(m_path)) m_watcher.addPath(m_path);
    // Watch existing ancestors so atomic replacement, theme symlink swaps and
    // initially missing directories all recover without polling.
    QString directory = QFileInfo(m_path).absolutePath();
    while (true) {
        if (QDir(directory).exists()) m_watcher.addPath(directory);
        const QString parent = QFileInfo(directory).absolutePath();
        if (parent == directory) break;
        directory = parent;
    }
}
