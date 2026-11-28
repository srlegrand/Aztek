// Aztek: Qt shell over the Rust host library.
#include "userfolder.h"
#include "external.h"
#include "aztek.h"
#include "tearoff.h"
#include "themes.h"
#include "host.h"
#include "mainwindow.h"

#include <QApplication>
#include <QSettings>
#include <cstring>
#include <QIcon>
#include <QSplashScreen>
#include <QElapsedTimer>
#include <QWindow>
#include "widgets.h"
#include <QCommandLineParser>
#include <QFile>
#include <QImage>
#include <QScreen>
#include <QJsonObject>
#include <QJsonDocument>
#include <QPalette>
#include <QStyleFactory>
#include <QTimer>
#include "viewport.h"
#ifdef Q_OS_WIN
#include <windows.h>
#endif

namespace {

// The reference's dark interface: mid-grey panels, darker fields, blue highlight.
void apply_style(QApplication& app) {
    // Aztek Classic, the reference's look, unless another theme was chosen.
    az::apply_saved_theme(app);
    az::install_tear_offs(app);
}

#ifdef Q_OS_WIN
// A window-subsystem program has no console of its own: batch output goes to the one it was started from.
void attach_console() {
    if (!AttachConsole(ATTACH_PARENT_PROCESS)) return;
    const DWORD which[] = {STD_OUTPUT_HANDLE, STD_ERROR_HANDLE};
    for (DWORD w : which) {
        HANDLE h = GetStdHandle(w);
        if (h == nullptr || h == INVALID_HANDLE_VALUE)
            SetStdHandle(w, CreateFileW(L"CONOUT$", GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE, nullptr, OPEN_EXISTING, 0, nullptr));
    }
    // aztekpy's prompt reads the console too.
    HANDLE in = GetStdHandle(STD_INPUT_HANDLE);
    if (in == nullptr || in == INVALID_HANDLE_VALUE)
        SetStdHandle(STD_INPUT_HANDLE, CreateFileW(L"CONIN$", GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE, nullptr, OPEN_EXISTING, 0, nullptr));
}
#endif

} // namespace

int main(int argc, char** argv) {
    // Batch rendering and batch scripts need no display: handled before Qt starts.
    for (int i = 1; i < argc; ++i)
        if (std::strcmp(argv[i], "-render") == 0) return aztek_render_batch(argc, argv);
    // aztekpy, the reference's mayapy: Python's command line with Aztek's modules and no window. As `aztek --python`,
    // or this program run under that name.
    {
        const char* name = argv[0];
        for (const char* c = argv[0]; *c; ++c)
            if (*c == '/' || *c == '\\') name = c + 1;
        bool python = std::strncmp(name, "aztekpy", 7) == 0;
        for (int i = 1; i < argc && !python; ++i) python = std::strcmp(argv[i], "--python") == 0 || std::strcmp(argv[i], "-python") == 0;
        if (python) {
#ifdef Q_OS_WIN
            attach_console();
#endif
            return aztek_python_main(argc, argv);
        }
    }
    for (int i = 1; i < argc; ++i)
        if (std::strcmp(argv[i], "--batch") == 0 || std::strcmp(argv[i], "-batch") == 0) {
#ifdef Q_OS_WIN
            attach_console();
#endif
            return aztek_batch(argc, argv);
        }
#ifdef Q_OS_LINUX
    // The viewport draws into X11 windows; on a Wayland session Qt runs through XWayland.
    if (qEnvironmentVariableIsEmpty("QT_QPA_PLATFORM")) {
        qputenv("QT_QPA_PLATFORM", "xcb");
        // So the browser and file manager Aztek starts are not told to use it too.
        qputenv("AZTEK_QPA_SET", "1");
    }
#endif
    QApplication::setAttribute(Qt::AA_DontCreateNativeWidgetSiblings);
    // Interface scaling as mayaDpiSetting left it (optionVars aztekDpiMode, aztekDpiScale): 0 by the system's DPI,
    // Qt's own; 1 a set scale; 2 none. The environment's own settings win.
    if (qEnvironmentVariableIsEmpty("QT_SCALE_FACTOR") && qEnvironmentVariableIsEmpty("QT_ENABLE_HIGHDPI_SCALING")) {
        const QString prefs = az::take(aztek_preferences_dir());
        QFile f(prefs + "/optionVars.json");
        if (f.open(QIODevice::ReadOnly)) {
            const QJsonObject o = QJsonDocument::fromJson(f.readAll()).object();
            const int mode = o["aztekDpiMode"].toInt(0);
            if (mode == 1) qputenv("QT_SCALE_FACTOR", QByteArray::number(o["aztekDpiScale"].toDouble(1.0)));
            if (mode == 2) qputenv("QT_ENABLE_HIGHDPI_SCALING", "0");
        }
    }
    QApplication app(argc, argv);
    if (QScreen* screen = QGuiApplication::primaryScreen()) qputenv("AZTEK_SYSTEM_DPI", QByteArray::number(screen->logicalDotsPerInch()));
    // The Qt this runs on, for about -qtVersion and the PySide check in Python.
    qputenv("AZTEK_QT_VERSION", qVersion());
#ifdef Q_OS_LINUX
    az::take(aztek_preferences_dir());
    QApplication::setOrganizationName("aztek");
#else
    QApplication::setOrganizationName("Aztek");
#endif
    QApplication::setApplicationName("Aztek");
    QApplication::setWindowIcon(QIcon(":/aztek-icon.png"));
    QGuiApplication::setDesktopFileName("aztek");
    apply_style(app);
    az::install_link_handler();
    // For about: the windowing platform and the interface font in use.
    qputenv("AZTEK_QT_PLATFORM", QGuiApplication::platformName().toUtf8());
    qputenv("AZTEK_FONT_INFO", QString("%1 %2pt").arg(QApplication::font().family()).arg(QApplication::font().pointSizeF()).toUtf8());
    // The views' samples a pixel, as chosen in a view's Lighting > Anti-aliasing. The environment's own value wins.
    if (qEnvironmentVariableIsEmpty("AZTEK_MSAA")) {
        const int samples = QSettings().value("viewport/samples", 4).toInt();
        if (samples == 1 || samples == 2 || samples == 4 || samples == 8) qputenv("AZTEK_MSAA", QByteArray::number(samples));
    }

    QCommandLineParser args;
    args.setApplicationDescription("Aztek");
    args.addHelpOption();
    QCommandLineOption script({"s", "script"}, "Run a MEL file at start.", "file");
    QCommandLineOption shot("screenshot", "Save a screenshot of the window after start-up and quit.", "png");
    QCommandLineOption viewport("viewport-screenshot", "Save the first visible viewport after start-up and quit.", "png");
    QCommandLineOption batch("batch", "No window: run the scene and scripts given, then exit with their status.");
    args.addOption(script);
    args.addOption(shot);
    args.addOption(viewport);
    args.addOption(batch);
    args.addPositionalArgument("scene", "Scene to open.");
    args.process(app);

    // The splash while the core and the viewports start, as the reference shows one.
    QSplashScreen* splash = nullptr;
    if (!args.isSet(shot)) {
        splash = new QSplashScreen(az::splash_pixmap());
        splash->show();
        // Drawn before the core starts and holds the event loop: a splash mapped but not yet painted showed what was
        // behind it on the screen (#225).
        QElapsedTimer exposed;
        exposed.start();
        while (!(splash->windowHandle() && splash->windowHandle()->isExposed()) && exposed.elapsed() < 1000) app.processEvents(QEventLoop::AllEvents, 20);
        splash->showMessage("Starting the core...", Qt::AlignBottom | Qt::AlignRight, QColor(150, 150, 150));
        app.processEvents();
    }
    // The saved preferences first: the up axis and units decide the first scene's cameras and grid.
    aztek_use_prefs();
    az::MainWindow w;
    aztek_set_main_window(static_cast<QMainWindow*>(&w));
    // The saved size and panels before the window is shown. Restoring them after show resized the window under the splash,
    // and under XWayland on a scaled screen the parts drawn at the old size stayed on screen (#225).
    if (!args.isSet(shot) && !args.isSet(viewport)) w.restore_saved_window();
    // A viewport capture is compared with a render frame: a fixed window keeps its size the same every time.
    if (args.isSet(viewport)) w.resize(1280, 800);
    // Keep the viewport hidden while the first-run dialog processes events. On macOS, presenting a frame here can
    // block the dialog while Metal waits for a drawable. Script and screenshot runs skip the prompt.
    if (!args.isSet(script) && !args.isSet(shot) && !args.isSet(viewport)) az::ask_user_folder_at_first_start(&w, splash);
    w.show();
    // A parity capture is one view, made before the start-up script so the script's lookThru reaches it.
    if (args.isSet(viewport)) w.panels()->set_layout(0);
    if (splash) {
        splash->finish(&w);
        delete splash;
        splash = nullptr;
        // The window drawn again where the splash was, before the start-up scripts hold the event loop (#225).
        w.repaint();
        app.processEvents();
    }
    // The plug-ins marked to load at start and the user folder's userSetup scripts, with the window there for the
    // menus and shelves they add.
    aztek_startup_scripts();
    if (!args.positionalArguments().isEmpty()) w.open_file(args.positionalArguments().first());
    w.run_startup(args.value(script), args.value(shot));
    if (!args.value(viewport).isEmpty()) {
        const QString path = args.value(viewport);
        QTimer::singleShot(1500, &w, [&w, path] {
            for (az::ViewportWidget* v : w.panels()->views()) {
                if (!v->isVisible()) continue;
                // A parity capture is the picture alone: the Realtime renderer, no grid over it.
                v->set("renderer", "\"realtime\"");
                v->set("grid", "false");
                v->set("viewCubeVisible", "false");
                // The picture alone: no light or camera icons, handles or selection highlights over it. Textures show,
                // as the Render View draws them.
                v->set("disp", "{\"lights\":false,\"cameras\":false,\"manipulators\":false,\"highlight\":false,\"filmGate\":false,\"resGate\":false,\"safeAction\":false,\"textured\":true,\"depthOfField\":true}");
                // The Render View's background is the scene's sky dome when it has one and black when it has none; the
                // viewport shows its own background for the dome, so the capture leaves it and forces black only
                // without a dome.
                v->set("parity", "true");
                // The host writes a PPM; the picture is kept as the format the path names.
                QString tmp = path + ".ppm";
                if (v->capture(tmp)) {
                    QImage(tmp).save(path);
                    QFile::remove(tmp);
                }
                break;
            }
            qApp->quit();
        });
    }
    int rc = app.exec();
    aztek_exiting();
    return rc;
}
