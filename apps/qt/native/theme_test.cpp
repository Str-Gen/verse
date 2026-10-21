#include "theme.h"

#include <QDBusVariant>
#include <QDir>
#include <QElapsedTimer>
#include <QGuiApplication>
#include <QSaveFile>
#include <QTemporaryDir>
#include <QThread>
#include <cstdio>
#include <functional>
#include <limits>

namespace {
void require(bool condition, const char *message) {
    if (!condition) { std::fprintf(stderr, "FAIL %s\n", message); std::exit(1); }
    std::printf("PASS %s\n", message);
}
bool eventually(const std::function<bool()> &predicate) {
    QElapsedTimer timer;
    timer.start();
    while (timer.elapsed() < 3000) {
        QCoreApplication::processEvents();
        if (predicate()) return true;
        QThread::msleep(10);
    }
    return false;
}
void replace(const QString &path, const QByteArray &contents) {
    QSaveFile file(path);
    require(file.open(QIODevice::WriteOnly), "open synthetic atomic theme file");
    require(file.write(contents) == contents.size() && file.commit(), "commit synthetic atomic theme file");
}
void deliverScale(Theme &theme, double scale) {
    const QDBusVariant value{QVariant(scale)};
    require(QMetaObject::invokeMethod(&theme, "portalChanged", Qt::DirectConnection,
        Q_ARG(QString, QString("org.gnome.desktop.interface")),
        Q_ARG(QString, QString("text-scaling-factor")),
        Q_ARG(QDBusVariant, value)), "invoke portal scale signal receiver locally");
}
}

int main(int argc, char **argv) {
    QGuiApplication app(argc, argv);
    QTemporaryDir temp(QDir::currentPath() + "/.local/theme-test-XXXXXX");
    require(temp.isValid(), "isolated temporary directory");
    const auto themePath = temp.path() + "/initially-missing/theme/colors.toml";
    qputenv("READER_THEME_FILE", themePath.toUtf8());
    Theme theme;
    int changes = 0;
    QObject::connect(&theme, &Theme::changed, [&changes] { ++changes; });
    require(QDir().mkpath(QFileInfo(themePath).absolutePath()), "create initially absent theme directories");
    replace(themePath, "mode = 'light'\nbackground = '#fdfcfb'\nforeground = '#121314'\naccent = '#123456'\n");
    require(eventually([&] { return theme.background() == QColor("#fdfcfb"); }),
        "watcher finds initially absent override file");
    require(theme.foreground() == QColor("#121314") && theme.accent() == QColor("#123456"),
        "parse quoted synthetic colors");
    replace(themePath, "mode = 'dark'\nbackground = '#112233' # comment\nforeground = '#eeeeee'\nselection_background = '#334455'\nselection = '#556677'\nselection_foreground = '#abcdef'\n");
    require(eventually([&] { return theme.background() == QColor("#112233"); }),
        "atomic replacement updates live colors");
    require(theme.selection() == QColor("#556677") && theme.selectionForeground() == QColor("#abcdef"),
        "selection and selected text color precedence");
    replace(themePath, "mode = 'light'\nbackground = 'invalid-color'\nforeground = '#111111'\naccent = 'invalid-color'\n");
    require(eventually([&] { return theme.background() == QColor("#ffffff"); }),
        "rearmed watcher sees second atomic replacement");
    require(theme.accent() == QColor("#2077b2") && theme.foreground() == QColor("#111111"),
        "invalid colors use mode defaults independently");
    deliverScale(theme, 1.75);
    require(qFuzzyCompare(theme.textScale(), 1.75), "portal receiver applies desktop text scale");
    deliverScale(theme, 9.0);
    require(qFuzzyCompare(theme.textScale(), 3.0), "portal receiver caps large text scale");
    deliverScale(theme, std::numeric_limits<double>::quiet_NaN());
    require(qFuzzyCompare(theme.textScale(), 1.0), "portal receiver rejects nonfinite scale");
    require(changes >= 6, "observable change notifications delivered");
    std::printf("PASS all theme checks; live portal transport was not simulated or modified\n");
}
