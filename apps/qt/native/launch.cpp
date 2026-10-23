#include "launch.h"
#include "theme.h"
#include "support.h"
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QTimer>
#include <QElapsedTimer>
#include <QQuickWindow>
#include <cstdio>
int run() {
    QElapsedTimer startup;
    startup.start();
    int argc = 1;
    char name[] = "verse-qt";
    char *argv[] = {name, nullptr};
    QGuiApplication app(argc, argv);
    if (qEnvironmentVariableIsSet("READER_SMOKE") || qEnvironmentVariableIsSet("READER_MEASURE"))
        std::fprintf(stderr, "READER_PLATFORM=%s\n", QGuiApplication::platformName().toUtf8().constData());
    app.setOrganizationName("OvernightPrototype");
    app.setApplicationName("QtReader");
    app.setApplicationDisplayName("Verse");
    QQuickStyle::setStyle("Basic");
    Theme theme;
    Support support;
    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty("theme", &theme);
    engine.rootContext()->setContextProperty("support", &support);
    engine.rootContext()->setContextProperty("performanceChapter", qEnvironmentVariableIntValue("READER_PERFORMANCE_CHAPTER"));
    engine.rootContext()->setContextProperty("smokeMode", qEnvironmentVariable("READER_SMOKE"));
    engine.rootContext()->setContextProperty("screenshotPath", qEnvironmentVariable("READER_SCREENSHOT"));
    QObject::connect(&engine, &QQmlApplicationEngine::quit, &app, &QCoreApplication::quit);
    engine.load(QUrl("qrc:/qt/qml/Overnight/Reader/qml/Main.qml"));
    if (engine.rootObjects().isEmpty()) return 2;
    if (qEnvironmentVariable("READER_SMOKE") == "page-performance") {
        auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
        if (window) QObject::connect(window, &QQuickWindow::frameSwapped, &support, &Support::framePresented, Qt::QueuedConnection);
    }
    if (qEnvironmentVariableIsSet("READER_MEASURE")) {
        auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
        if (window) {
            QObject::connect(window, &QQuickWindow::frameSwapped, &app, [&startup]() {
                std::fprintf(stderr, "READER_FIRST_FRAME_MS=%lld\n", static_cast<long long>(startup.elapsed()));
            }, Qt::SingleShotConnection);
            QObject::connect(window, &QQuickWindow::frameSwapped, &app, [window, &startup, reported = false]() mutable {
                if (!reported && window->property("bookReady").toBool()) {
                    reported = true;
                    std::fprintf(stderr, "READER_BOOK_READY_MS=%lld\n", static_cast<long long>(startup.elapsed()));
                }
            });
        }
    }
    if (qEnvironmentVariableIsSet("READER_SMOKE"))
        QTimer::singleShot(qEnvironmentVariable("READER_SMOKE") == "page-performance" ? 120000 : 15000, &app, [&app]() { app.exit(90); });
    return app.exec();
}
