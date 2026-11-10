// The Render View: Harold's image as it refines, interactive rendering, a region, kept images compared by a wipe, AOVs,
// exposure and gamma, and saving. Also Render Sequence, which runs `aztek -render` in its own process, the Render and
// Harold menus, and Create > Harold. Nothing here waits: the view polls the renderer on a timer.
#include "renderui.h"
#include "aztek.h"
#include "host.h"
#include "icons.h"
#include "panels.h"

#include <QAction>
#include <QApplication>
#include <QCheckBox>
#include <QComboBox>
#include <QCoreApplication>
#include <QDialog>
#include <QDialogButtonBox>
#include <QDir>
#include <QDoubleSpinBox>
#include <QElapsedTimer>
#include <QFile>
#include <QFileDialog>
#include <QFileInfo>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QJsonArray>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QMenu>
#include <QMenuBar>
#include <QMouseEvent>
#include <QPainter>
#include <QPlainTextEdit>
#include <QPointer>
#include <QProcess>
#include <QProgressBar>
#include <QPushButton>
#include <QSet>
#include <QSpinBox>
#include <QTemporaryDir>
#include <QTimer>
#include <QToolBar>
#include <QToolButton>
#include <QVBoxLayout>
#include <QWheelEvent>
#include <cmath>
#include <memory>

namespace az {

namespace {

QJsonObject one(const QString& a) {
    return QJsonObject{{"args", QJsonArray{a}}};
}

// An AOV of the current render, or a kept image (snapshot >= 0), as the renderer displays it.
QImage fetch(const QString& aov, int snapshot, double exposure, double gamma) {
    int w = 0, h = 0;
    QByteArray a = aov.toUtf8();
    unsigned char* d = aztek_render_image(a.constData(), snapshot, exposure, gamma, &w, &h);
    if (!d) return QImage();
    QImage out = QImage(d, w, h, QImage::Format_RGBA8888).copy();
    aztek_image_free(d, w, h);
    return out;
}

QString rate(double x) {
    if (x >= 1e9) return QString::number(x / 1e9, 'f', 2) + " G";
    if (x >= 1e6) return QString::number(x / 1e6, 'f', 1) + " M";
    if (x >= 1e3) return QString::number(x / 1e3, 'f', 1) + " K";
    return QString::number(x, 'f', 0) + " ";
}

QString clock(double s) {
    int m = int(s / 60);
    return QString("%1:%2").arg(m).arg(s - m * 60, 4, 'f', 1, QChar('0'));
}

// The last render the menus started, for Redo Previous Render.
struct LastRender {
    QJsonObject call;
    bool ipr = false;
    bool any = false;
};
LastRender& last_render() {
    static LastRender l;
    return l;
}

// ---------------------------------------------------------------------------------------------------------------------
// The image: fit or zoomed and panned, a region dragged on it, and a kept image to the right of a divider.

class ImageView : public QWidget {
public:
    explicit ImageView(QWidget* parent) : QWidget(parent) {
        setMinimumSize(320, 200);
        setMouseTracking(true);
        setFocusPolicy(Qt::ClickFocus);
    }
    QImage a, b;
    bool wipe = false;
    double split = 0.5;
    bool region_mode = false;
    QRect region;
    QSize full;
    std::function<void(QRect)> on_region;

    void set_fit() {
        fit_ = true;
        update();
    }
    void set_one() {
        fit_ = false;
        scale_ = 1.0;
        pan_ = QPointF();
        update();
    }

protected:
    QSize image_size() const { return !a.isNull() ? a.size() : full; }
    double scale() const {
        QSize s = image_size();
        if (s.isEmpty()) return 1.0;
        if (fit_) return qMin(double(width() - 8) / s.width(), double(height() - 8) / s.height());
        return scale_;
    }
    QRectF target() const {
        QSize s = image_size();
        double k = scale();
        QPointF c = QPointF(width() / 2.0, height() / 2.0) + (fit_ ? QPointF() : pan_);
        return QRectF(c.x() - s.width() * k / 2, c.y() - s.height() * k / 2, s.width() * k, s.height() * k);
    }
    QPointF to_image(QPointF w) const {
        QRectF t = target();
        double k = scale();
        return (w - t.topLeft()) / k;
    }
    double split_x() const {
        QRectF t = target();
        return t.left() + t.width() * split;
    }

    void paintEvent(QPaintEvent*) override {
        QPainter p(this);
        p.fillRect(rect(), QColor(38, 38, 38));
        QSize s = image_size();
        if (s.isEmpty()) {
            p.setPen(QColor(150, 150, 150));
            p.drawText(rect(), Qt::AlignCenter, "Render or start IPR to see the image here.");
            return;
        }
        QRectF t = target();
        p.setRenderHint(QPainter::SmoothPixmapTransform, scale() < 1.0);
        // A checker under the image shows where it is transparent.
        p.fillRect(t, QColor(60, 60, 60));
        if (!a.isNull()) p.drawImage(t, a);
        if (wipe && !b.isNull()) {
            double x = split_x();
            p.save();
            p.setClipRect(QRectF(x, t.top(), t.right() - x, t.height()));
            p.drawImage(t, b);
            p.restore();
            p.setPen(QPen(QColor(240, 240, 240), 1.5));
            p.drawLine(QPointF(x, t.top()), QPointF(x, t.bottom()));
            p.setBrush(QColor(240, 240, 240));
            p.drawEllipse(QPointF(x, t.center().y()), 5, 5);
            p.setPen(QColor(240, 240, 240));
            p.drawText(QRectF(x - 40, t.top() + 4, 34, 16), Qt::AlignRight, "A");
            p.drawText(QRectF(x + 6, t.top() + 4, 34, 16), Qt::AlignLeft, "B");
        }
        QRect r = dragging_region_ ? QRect(drag_from_.toPoint(), drag_to_.toPoint()).normalized() : region;
        if (!r.isEmpty() && (region_mode || dragging_region_)) {
            double k = scale();
            QRectF rr(t.left() + r.left() * k, t.top() + r.top() * k, r.width() * k, r.height() * k);
            p.setBrush(Qt::NoBrush);
            p.setPen(QPen(QColor(230, 70, 60), 1.5, Qt::DashLine));
            p.drawRect(rr);
        }
    }

    void mousePressEvent(QMouseEvent* e) override {
        last_ = e->position();
        if (e->button() == Qt::MiddleButton || (e->button() == Qt::LeftButton && (e->modifiers() & Qt::AltModifier))) {
            if (fit_) {
                scale_ = scale();
                pan_ = QPointF();
                fit_ = false;
            }
            panning_ = true;
            return;
        }
        if (e->button() != Qt::LeftButton) return;
        if (wipe && !b.isNull() && std::abs(e->position().x() - split_x()) < 8) {
            dragging_split_ = true;
            return;
        }
        if (region_mode) {
            dragging_region_ = true;
            drag_from_ = drag_to_ = clamp_image(to_image(e->position()));
            update();
        }
    }
    void mouseMoveEvent(QMouseEvent* e) override {
        if (panning_) {
            pan_ += e->position() - last_;
            last_ = e->position();
            update();
        } else if (dragging_split_) {
            QRectF t = target();
            split = qBound(0.0, (e->position().x() - t.left()) / qMax(1.0, t.width()), 1.0);
            update();
        } else if (dragging_region_) {
            drag_to_ = clamp_image(to_image(e->position()));
            update();
        } else {
            setCursor(wipe && !b.isNull() && std::abs(e->position().x() - split_x()) < 8 ? Qt::SplitHCursor : region_mode ? Qt::CrossCursor : Qt::ArrowCursor);
        }
    }
    void mouseReleaseEvent(QMouseEvent*) override {
        panning_ = dragging_split_ = false;
        if (dragging_region_) {
            dragging_region_ = false;
            QRect r = QRect(drag_from_.toPoint(), drag_to_.toPoint()).normalized();
            if (r.width() > 2 && r.height() > 2) {
                region = r;
                if (on_region) on_region(r);
            }
            update();
        }
    }
    void wheelEvent(QWheelEvent* e) override {
        double k0 = scale();
        if (fit_) {
            scale_ = k0;
            pan_ = QPointF();
            fit_ = false;
        }
        QPointF at = to_image(e->position());
        scale_ = qBound(0.05, scale_ * (e->angleDelta().y() > 0 ? 1.25 : 0.8), 32.0);
        // Keep the pixel under the cursor where it is.
        QRectF t = target();
        QPointF now = t.topLeft() + at * scale_;
        pan_ += e->position() - now;
        update();
    }
    void mouseDoubleClickEvent(QMouseEvent*) override { set_fit(); }

private:
    QPointF clamp_image(QPointF p) const {
        QSize s = image_size();
        return QPointF(qBound(0.0, p.x(), double(s.width())), qBound(0.0, p.y(), double(s.height())));
    }
    bool fit_ = true;
    double scale_ = 1.0;
    QPointF pan_;
    QPointF last_;
    bool panning_ = false, dragging_split_ = false, dragging_region_ = false;
    QPointF drag_from_, drag_to_;
};

// ---------------------------------------------------------------------------------------------------------------------

class RenderView : public QWidget {
public:
    explicit RenderView(QWidget* parent) : QWidget(parent, editor_window_flags()) {
        setWindowTitle("Harold RenderView");
        setObjectName("haroldRenderView");
        auto* v = new QVBoxLayout(this);
        v->setContentsMargins(0, 0, 0, 0);
        v->setSpacing(0);

        auto* bar = new QMenuBar(this);
        bar->setNativeMenuBar(false);
        QMenu* file = bar->addMenu("File");
        file->addAction("Save Image...", [this] { save_image(); });
        file->addSeparator();
        file->addAction("Close", [this] { close(); });
        QMenu* render = bar->addMenu("Render");
        render->addAction("Render", [this] { render_now(); });
        render->addAction("IPR", [this] { ipr_->toggle(); });
        render->addAction("Refresh", [this] { refresh_render(); });
        render->addAction("Stop", [this] { stop(); });
        render->addSeparator();
        render->addAction("Render Settings...", [this] { show_render_settings(parentWidget()); });
        QMenu* view = bar->addMenu("View");
        view->addAction("Frame Image", [this] { view_->set_fit(); });
        view->addAction("Real Size", [this] { view_->set_one(); });
        view->addSeparator();
        view->addAction("Reset Exposure and Gamma", [this] {
            exposure_->setValue(0.0);
            gamma_->setValue(1.0);
        });
        QMenu* keep = bar->addMenu("Snapshots");
        keep->addAction("Keep Image", [this] { snapshot(); });
        keep->addAction("Remove All Kept Images", [] { exec("renderWindowEditor", QJsonObject{{"args", QJsonArray{"renderView"}}, {"edit", true}, {"removeAllImages", true}}); });
        v->setMenuBar(bar);

        auto* tb = new QToolBar(this);
        tb->setIconSize(QSize(22, 22));
        tb->addAction(icon("renderFrame"), "Render the current frame", [this] { render_now(); });
        ipr_ = tb->addAction(icon("renderIpr"), "IPR: keep rendering as the scene changes");
        ipr_->setCheckable(true);
        connect(ipr_, &QAction::toggled, this, [this](bool on) { set_ipr(on); });
        tb->addAction(icon("renderRefresh"), "Refresh: render again", [this] { refresh_render(); });
        tb->addAction(icon("renderStop"), "Stop rendering", [this] { stop(); });
        tb->addSeparator();
        region_on_ = tb->addAction(icon("renderRegion"), "Region: drag a box on the image, then only that renders");
        region_on_->setCheckable(true);
        connect(region_on_, &QAction::toggled, this, [this](bool on) {
            view_->region_mode = on;
            view_->update();
            if (!on && ipr_->isChecked()) refresh_render();
        });
        tb->addSeparator();
        tb->addAction(icon("snapshot"), "Keep Image: a snapshot to compare against", [this] { snapshot(); });
        wipe_ = tb->addAction(icon("renderWipe"), "A/B wipe: the image left of the divider, the chosen snapshot right of it");
        wipe_->setCheckable(true);
        connect(wipe_, &QAction::toggled, this, [this](bool) { update_b(); });
        tb->addSeparator();
        tb->addWidget(new QLabel(" Camera ", tb));
        camera_ = new QComboBox(tb);
        camera_->setMinimumWidth(110);
        camera_->setToolTip("The camera to render through");
        tb->addWidget(camera_);
        tb->addWidget(new QLabel(" AOV ", tb));
        aov_ = new QComboBox(tb);
        aov_->setMinimumWidth(110);
        aov_->addItem("RGBA");
        aov_->addItem("denoised");
        connect(aov_, &QComboBox::currentTextChanged, this, [this] { fetch_now(); });
        tb->addWidget(aov_);
        tb->addWidget(new QLabel(" Exposure ", tb));
        exposure_ = new QDoubleSpinBox(tb);
        exposure_->setRange(-10, 10);
        exposure_->setSingleStep(0.5);
        exposure_->setDecimals(2);
        exposure_->setToolTip("Display exposure, in stops");
        tb->addWidget(exposure_);
        tb->addWidget(new QLabel(" Gamma ", tb));
        gamma_ = new QDoubleSpinBox(tb);
        gamma_->setRange(0.1, 5);
        gamma_->setSingleStep(0.1);
        gamma_->setDecimals(2);
        gamma_->setValue(1.0);
        gamma_->setToolTip("Display gamma");
        tb->addWidget(gamma_);
        for (QDoubleSpinBox* s : {exposure_, gamma_})
            connect(s, &QDoubleSpinBox::valueChanged, this, [this] {
                exec_quiet("renderWindowEditor", QJsonObject{{"args", QJsonArray{"renderView"}}, {"edit", true}, {"exposure", exposure_->value()}, {"gamma", gamma_->value()}});
                fetch_now();
                update_b();
            });
        tb->addSeparator();
        tb->addAction(icon("zoomFit"), "Frame the image", [this] { view_->set_fit(); });
        tb->addAction(icon("zoomOne"), "Real size, one image pixel to one screen pixel", [this] { view_->set_one(); });
        tb->addAction(icon("saveImage"), "Save Image", [this] { save_image(); });
        v->addWidget(tb);

        view_ = new ImageView(this);
        view_->on_region = [this](QRect) {
            if (!region_on_->isChecked()) return;
            if (ipr_->isChecked()) refresh_render();
            else render_now();
        };
        v->addWidget(view_, 1);

        snaps_ = new QListWidget(this);
        snaps_->setViewMode(QListView::IconMode);
        snaps_->setFlow(QListView::LeftToRight);
        snaps_->setWrapping(false);
        snaps_->setMovement(QListView::Static);
        snaps_->setIconSize(QSize(80, 50));
        snaps_->setGridSize(QSize(100, 76));
        snaps_->setFixedHeight(88);
        snaps_->setToolTip("Kept images. Pick one and turn on the A/B wipe to compare. Right click to remove.");
        snaps_->setContextMenuPolicy(Qt::CustomContextMenu);
        snaps_->hide();
        connect(snaps_, &QListWidget::currentRowChanged, this, [this] { update_b(); });
        connect(snaps_, &QListWidget::itemDoubleClicked, this, [this] { wipe_->setChecked(true); });
        connect(snaps_, &QWidget::customContextMenuRequested, this, [this](const QPoint& at) {
            QListWidgetItem* it = snaps_->itemAt(at);
            QMenu m;
            if (it) {
                int k = snaps_->row(it);
                m.addAction("Compare (A/B Wipe)", [this, k] {
                    snaps_->setCurrentRow(k);
                    wipe_->setChecked(true);
                });
                m.addAction("Remove", [k] { exec("renderWindowEditor", QJsonObject{{"args", QJsonArray{"renderView"}}, {"edit", true}, {"removeImage", k}}); });
            }
            m.addAction("Remove All", [] { exec("renderWindowEditor", QJsonObject{{"args", QJsonArray{"renderView"}}, {"edit", true}, {"removeAllImages", true}}); });
            m.exec(snaps_->mapToGlobal(at));
        });
        v->addWidget(snaps_);

        status_ = new QLabel(this);
        status_->setContentsMargins(6, 3, 6, 3);
        status_->setTextInteractionFlags(Qt::TextSelectableByMouse);
        v->addWidget(status_);

        timer_.setInterval(150);
        connect(&timer_, &QTimer::timeout, this, [this] { poll(); });
        connect(Host::get(), &Host::sceneChanged, this, [this] {
            if (isVisible()) fill_cameras();
        });
        resize(980, 700);
    }

    // What the toolbar adds to a render call: the camera and the region.
    QJsonObject call() const {
        QJsonObject c;
        QString cam = camera_->currentData().toString();
        if (!cam.isEmpty()) c["camera"] = cam;
        QRect r = view_->region;
        if (region_on_->isChecked() && !r.isEmpty()) c["region"] = QJsonArray{r.left(), r.top(), r.right() + 1, r.bottom() + 1};
        return c;
    }

    void render_now() {
        if (ipr_->isChecked()) {
            QSignalBlocker b(ipr_);
            ipr_->setChecked(false);
        }
        QJsonObject c = call();
        last_render() = LastRender{c, false, true};
        c["async"] = true;
        exec("hrender", c);
        fetch_now();
    }
    void set_ipr(bool on) {
        QJsonObject c = on ? call() : QJsonObject();
        if (on) last_render() = LastRender{c, true, true};
        c["ipr"] = on ? "start" : "stop";
        exec("hrender", c);
        fetch_now();
    }
    void start_ipr() {
        if (ipr_->isChecked()) refresh_render();
        else ipr_->setChecked(true);
    }
    void refresh_render() {
        if (!ipr_->isChecked()) {
            render_now();
            return;
        }
        QJsonObject c = call();
        c["ipr"] = "refresh";
        exec("hrender", c);
        fetch_now();
    }
    void redo(const LastRender& l) {
        if (l.ipr) {
            if (!ipr_->isChecked()) {
                QSignalBlocker b(ipr_);
                ipr_->setChecked(true);
            }
            QJsonObject c = l.call;
            c["ipr"] = "start";
            exec("hrender", c);
        } else {
            QJsonObject c = l.call;
            c["async"] = true;
            exec("hrender", c);
        }
        fetch_now();
    }
    void stop() {
        if (ipr_->isChecked()) {
            QSignalBlocker b(ipr_);
            ipr_->setChecked(false);
        }
        exec("hrender", QJsonObject{{"ipr", "stop"}});
    }
    void snapshot() {
        exec("renderWindowEditor", QJsonObject{{"args", QJsonArray{"renderView"}}, {"edit", true}, {"snapShot", true}});
        snap_count_ = -1;
        poll();
    }
    void save_image() {
        QString filter;
        QString path = QFileDialog::getSaveFileName(this, "Save Image", QDir::homePath() + "/render.exr",
                                                    "OpenEXR, every AOV (*.exr);;PNG (*.png);;TIFF (*.tif *.tiff);;JPEG (*.jpg *.jpeg);;Targa (*.tga)", &filter);
        if (path.isEmpty()) return;
        if (QFileInfo(path).suffix().isEmpty()) {
            static const QList<QPair<QString, QString>> ext{{"exr", ".exr"}, {"png", ".png"}, {"tif", ".tif"}, {"jpg", ".jpg"}, {"tga", ".tga"}};
            for (const auto& e : ext)
                if (filter.contains("*." + e.first)) {
                    path += e.second;
                    break;
                }
        }
        QJsonValue files = exec("renderWindowEditor", QJsonObject{{"args", QJsonArray{"renderView"}}, {"edit", true}, {"writeImage", path}});
        if (files.isArray()) saved_ = QString("Saved %1 file%2").arg(files.toArray().size()).arg(files.toArray().size() == 1 ? "" : "s");
    }

protected:
    void showEvent(QShowEvent* e) override {
        QWidget::showEvent(e);
        fill_cameras();
        timer_.start();
        poll();
    }
    void hideEvent(QHideEvent* e) override {
        QWidget::hideEvent(e);
        timer_.stop();
    }

private:
    void fill_cameras() {
        QStringList cams;
        for (const QJsonValue& c : ask("listCameras").toArray()) cams << c.toString();
        if (cams == cameras_) return;
        cameras_ = cams;
        QString keep = camera_->currentData().toString();
        camera_->clear();
        camera_->addItem("Renderable camera", QString());
        for (const QString& c : cams) camera_->addItem(c.section('|', -1), c);
        int k = camera_->findData(keep);
        camera_->setCurrentIndex(k < 0 ? 0 : k);
    }

    void fetch_now() { force_ = true; }

    void update_b() {
        view_->wipe = wipe_->isChecked();
        int k = snaps_->currentRow();
        if (view_->wipe && k < 0 && snaps_->count()) {
            k = snaps_->count() - 1;
            QSignalBlocker blk(snaps_);
            snaps_->setCurrentRow(k);
        }
        view_->b = view_->wipe && k >= 0 ? fetch("RGBA", k, exposure_->value(), gamma_->value()) : QImage();
        view_->update();
    }

    void poll() {
        if (!isVisible()) return;
        QJsonObject st = parse(take(aztek_render_status())).toObject();
        if (st.isEmpty()) return;
        {
            QSignalBlocker b(ipr_);
            ipr_->setChecked(st["ipr"].toBool());
        }
        // AOVs: the beauty, what the render made, and a denoised beauty.
        QStringList aovs;
        for (const QJsonValue& a : st["aovs"].toArray()) aovs << a.toString();
        if (!aovs.contains("RGBA")) aovs.prepend("RGBA");
        aovs << "denoised";
        if (aovs != aov_names_) {
            aov_names_ = aovs;
            QSignalBlocker b(aov_);
            QString keep = aov_->currentText();
            aov_->clear();
            aov_->addItems(aovs);
            int k = aov_->findText(keep);
            aov_->setCurrentIndex(k < 0 ? 0 : k);
        }
        view_->full = QSize(st["width"].toInt(), st["height"].toInt());
        if (st.contains("restartSeconds")) {
            restart_ = st["restartSeconds"].toDouble();
            force_ = true;
        }
        QString key = QString("%1/%2/%3/%4/%5x%6/%7/%8").arg(st["pass"].toInt()).arg(st["passes"].toInt()).arg(st["preview"].toVariant().toString()).arg(st["done"].toBool()).arg(st["width"].toInt()).arg(st["height"].toInt()).arg(st["running"].toBool()).arg(st["camera"].toString());
        QString aov = aov_->currentText();
        bool denoised = aov == "denoised";
        if (key != key_ || force_) {
            // Denoising the whole image is slow: while it refines, at most once a second and a half.
            bool now = !denoised || force_ || st["done"].toBool() || !denoise_clock_.isValid() || denoise_clock_.elapsed() > 1500;
            if (now) {
                key_ = key;
                force_ = false;
                QImage img = fetch(aov, -1, exposure_->value(), gamma_->value());
                // A region render fills only its box: keep the last image around it, as the render view does.
                QRect r = view_->region;
                if (!img.isNull() && region_on_->isChecked() && !r.isEmpty() && view_->a.size() == img.size()) {
                    QImage keep = view_->a;
                    QPainter p(&keep);
                    p.setCompositionMode(QPainter::CompositionMode_Source);
                    p.drawImage(r.topLeft(), img, r);
                    p.end();
                    img = keep;
                }
                if (!img.isNull() || !st["running"].toBool()) view_->a = img;
                if (denoised) denoise_clock_.restart();
                view_->update();
            }
        }
        // Kept images.
        QJsonArray snaps = st["snapshots"].toArray();
        if (snaps.size() != snap_count_) {
            snap_count_ = snaps.size();
            QSignalBlocker b(snaps_);
            int keep = snaps_->currentRow();
            snaps_->clear();
            for (int k = 0; k < snaps.size(); ++k) {
                QJsonObject o = snaps.at(k).toObject();
                QImage img = fetch("RGBA", k, exposure_->value(), gamma_->value());
                QPixmap px = QPixmap::fromImage(img.scaled(80, 50, Qt::KeepAspectRatio, Qt::SmoothTransformation));
                auto* it = new QListWidgetItem(QIcon(px), o["name"].toString(), snaps_);
                it->setToolTip(o["info"].toString());
            }
            snaps_->setVisible(!snaps.isEmpty());
            snaps_->setCurrentRow(keep < snaps.size() ? keep : snaps.size() - 1);
            if (snaps.isEmpty()) wipe_->setChecked(false);
            update_b();
        }
        // Warnings go to the Script Editor once each, oldest first.
        QJsonArray warns = st["warnings"].toArray();
        for (int k = warns.size() - 1; k >= 0; --k) {
            QString w = warns.at(k).toString();
            if (w.isEmpty() || warned_.contains(w)) continue;
            warned_.insert(w);
            log("// Warning: Harold: " + w + " //", 4);
        }
        // The status line.
        QStringList parts;
        bool running = st["running"].toBool();
        int pass = st["pass"].toInt(), passes = st["passes"].toInt();
        if (running) parts << QString("%1 pass %2 of %3").arg(st["ipr"].toBool() ? "IPR" : "Rendering").arg(pass).arg(passes);
        else if (st["done"].toBool()) parts << QString("Done, %1 passes").arg(passes);
        else if (st["ipr"].toBool()) parts << "IPR waiting";
        else parts << "Idle";
        if (st["width"].toInt()) parts << QString("%1 x %2").arg(st["width"].toInt()).arg(st["height"].toInt());
        if (!st["camera"].toString().isEmpty()) parts << st["camera"].toString().section('|', -1);
        if (st["seconds"].toDouble() > 0) parts << clock(st["seconds"].toDouble());
        if (st["raysPerSecond"].toDouble() > 0) parts << rate(st["raysPerSecond"].toDouble()) + "rays/s";
        if (st["memoryMB"].toDouble() > 0) parts << QString("%1 MB").arg(st["memoryMB"].toDouble(), 0, 'f', 0);
        if (st["translateSeconds"].toDouble() > 0) parts << QString("scene %1 s").arg(st["translateSeconds"].toDouble(), 0, 'f', 2);
        if (restart_ >= 0) parts << QString("IPR restart %1 ms").arg(restart_ * 1000.0, 0, 'f', 0);
        if (!saved_.isEmpty()) parts << saved_;
        status_->setText(parts.join("   |   "));
    }

    ImageView* view_ = nullptr;
    QAction* ipr_ = nullptr;
    QAction* region_on_ = nullptr;
    QAction* wipe_ = nullptr;
    QComboBox* camera_ = nullptr;
    QComboBox* aov_ = nullptr;
    QDoubleSpinBox* exposure_ = nullptr;
    QDoubleSpinBox* gamma_ = nullptr;
    QListWidget* snaps_ = nullptr;
    QLabel* status_ = nullptr;
    QTimer timer_;
    QString key_;
    bool force_ = true;
    int snap_count_ = -1;
    double restart_ = -1;
    QSet<QString> warned_;
    QStringList aov_names_, cameras_;
    QElapsedTimer denoise_clock_;
    QString saved_;
};

QPointer<RenderView> g_view;

RenderView* view(QWidget* parent) {
    if (!g_view) g_view = new RenderView(parent);
    g_view->show();
    g_view->raise();
    return g_view;
}

// ---------------------------------------------------------------------------------------------------------------------
// Render Sequence: the scene written to a temporary file and rendered frame by frame by `aztek -render`.

class SequenceDialog : public QDialog {
public:
    explicit SequenceDialog(QWidget* parent) : QDialog(parent) {
        setWindowTitle("Render Sequence");
        setAttribute(Qt::WA_DeleteOnClose);
        auto* v = new QVBoxLayout(this);
        auto* form = new QFormLayout();
        auto num = [](const QString& plug, double fallback) {
            QJsonValue x = ask("getAttr", one(plug));
            return x.isDouble() ? x.toDouble() : fallback;
        };
        start_ = new QDoubleSpinBox(this);
        end_ = new QDoubleSpinBox(this);
        by_ = new QDoubleSpinBox(this);
        for (QDoubleSpinBox* s : {start_, end_, by_}) {
            s->setRange(-100000, 100000);
            s->setDecimals(2);
        }
        start_->setValue(num("defaultRenderGlobals.startFrame", 1));
        end_->setValue(num("defaultRenderGlobals.endFrame", 10));
        by_->setValue(num("defaultRenderGlobals.byFrameStep", 1));
        by_->setMinimum(0.01);
        auto* frames = new QHBoxLayout();
        frames->addWidget(start_);
        frames->addWidget(new QLabel("to", this));
        frames->addWidget(end_);
        frames->addWidget(new QLabel("by", this));
        frames->addWidget(by_);
        form->addRow("Frames", frames);
        camera_ = new QComboBox(this);
        camera_->addItem("Renderable camera", QString());
        for (const QJsonValue& c : ask("listCameras").toArray()) camera_->addItem(c.toString().section('|', -1), c.toString().section('|', -1));
        form->addRow("Camera", camera_);
        width_ = new QSpinBox(this);
        height_ = new QSpinBox(this);
        for (QSpinBox* s : {width_, height_}) s->setRange(1, 32768);
        width_->setValue(int(num("defaultResolution.width", 960)));
        height_->setValue(int(num("defaultResolution.height", 540)));
        auto* size = new QHBoxLayout();
        size->addWidget(width_);
        size->addWidget(new QLabel("x", this));
        size->addWidget(height_);
        form->addRow("Size", size);
        format_ = new QComboBox(this);
        format_->addItems({"exr", "png", "tif", "jpg", "tga"});
        QString fmt = ask("getAttr", one("defaultArnoldDriver.ai_translator")).toString();
        if (format_->findText(fmt) >= 0) format_->setCurrentText(fmt);
        form->addRow("Format", format_);
        dir_ = new QLineEdit(this);
        QString first = ask("renderSettings", QJsonObject{{"firstImageName", true}, {"fullPath", true}}).toArray().at(0).toString();
        dir_->setText(first.isEmpty() ? QDir::homePath() + "/images" : QFileInfo(first).absolutePath());
        auto* browse = new QToolButton(this);
        browse->setIcon(icon("open", 16));
        connect(browse, &QToolButton::clicked, this, [this] {
            QString d = QFileDialog::getExistingDirectory(this, "Output Folder", dir_->text());
            if (!d.isEmpty()) dir_->setText(d);
        });
        auto* out = new QHBoxLayout();
        out->addWidget(dir_, 1);
        out->addWidget(browse);
        form->addRow("Output folder", out);
        v->addLayout(form);
        progress_ = new QProgressBar(this);
        progress_->setValue(0);
        v->addWidget(progress_);
        log_ = new QPlainTextEdit(this);
        log_->setReadOnly(true);
        log_->setMinimumHeight(140);
        v->addWidget(log_, 1);
        auto* buttons = new QHBoxLayout();
        go_ = new QPushButton("Render", this);
        cancel_ = new QPushButton("Cancel", this);
        cancel_->setEnabled(false);
        auto* close = new QPushButton("Close", this);
        buttons->addStretch(1);
        buttons->addWidget(go_);
        buttons->addWidget(cancel_);
        buttons->addWidget(close);
        v->addLayout(buttons);
        connect(go_, &QPushButton::clicked, this, [this] { begin(); });
        connect(cancel_, &QPushButton::clicked, this, [this] { cancel(); });
        connect(close, &QPushButton::clicked, this, &QDialog::close);
        resize(520, 460);
    }
    ~SequenceDialog() override { cancel(); }

private:
    void say(const QString& s) { log_->appendPlainText(s); }

    void begin() {
        frames_.clear();
        for (double f = start_->value(); f <= end_->value() + 1e-9; f += by_->value()) frames_ << f;
        if (frames_.isEmpty()) return;
        tmp_ = std::make_unique<QTemporaryDir>();
        if (!tmp_->isValid()) {
            say("Could not make a temporary folder.");
            return;
        }
        // Named after the scene, so <Scene> in the file name prefix still gives the scene's name.
        QString scene = state()["file"].toString();
        QString base = scene.isEmpty() ? QString("untitled") : QFileInfo(scene).completeBaseName();
        file_ = tmp_->filePath(base + ".ma");
        exec_quiet("file", QJsonObject{{"args", QJsonArray{file_}}, {"exportAll", true}, {"type", "mayaAscii"}});
        if (!QFileInfo::exists(file_)) {
            say("Could not write the scene for rendering.");
            return;
        }
        // Each frame gets its number in the file name, even when the settings say a single frame.
        {
            QFile f(file_);
            if (f.open(QIODevice::Append | QIODevice::Text)) f.write("setAttr \"defaultRenderGlobals.animation\" yes;\n");
        }
        QDir().mkpath(dir_->text());
        done_ = 0;
        progress_->setRange(0, frames_.size());
        progress_->setValue(0);
        go_->setEnabled(false);
        cancel_->setEnabled(true);
        clock_.start();
        say(QString("Rendering %1 frame%2 to %3").arg(frames_.size()).arg(frames_.size() == 1 ? "" : "s").arg(dir_->text()));
        next();
    }

    void next() {
        if (done_ >= frames_.size()) {
            say(QString("Finished in %1 s.").arg(clock_.elapsed() / 1000.0, 0, 'f', 1));
            go_->setEnabled(true);
            cancel_->setEnabled(false);
            return;
        }
        double f = frames_.at(done_);
        QStringList a{"-render", "-s", QString::number(f), "-e", QString::number(f), "-x", QString::number(width_->value()), "-y", QString::number(height_->value()), "-rd", dir_->text(), "-of", format_->currentText()};
        QString cam = camera_->currentData().toString();
        if (!cam.isEmpty()) a << "-cam" << cam;
        a << file_;
        proc_ = new QProcess(this);
        connect(proc_, &QProcess::readyReadStandardOutput, this, [this, p = proc_] {
            for (const QString& l : QString::fromUtf8(p->readAllStandardOutput()).split('\n', Qt::SkipEmptyParts)) say("Wrote " + l);
        });
        connect(proc_, &QProcess::readyReadStandardError, this, [this, p = proc_] {
            for (const QString& l : QString::fromUtf8(p->readAllStandardError()).split('\n', Qt::SkipEmptyParts)) say(l);
        });
        connect(proc_, &QProcess::finished, this, [this, p = proc_, f](int code, QProcess::ExitStatus how) {
            p->deleteLater();
            if (proc_ != p) return;
            proc_ = nullptr;
            if (how != QProcess::NormalExit || code != 0) {
                say(QString("Frame %1 failed (exit code %2).").arg(f).arg(code));
                go_->setEnabled(true);
                cancel_->setEnabled(false);
                return;
            }
            ++done_;
            progress_->setValue(done_);
            double each = clock_.elapsed() / 1000.0 / done_;
            progress_->setFormat(QString("%v of %m, about %1 s left").arg(each * (frames_.size() - done_), 0, 'f', 0));
            next();
        });
        proc_->start(QCoreApplication::applicationFilePath(), a);
    }

    void cancel() {
        if (proc_) {
            QProcess* p = proc_;
            proc_ = nullptr;
            p->kill();
            p->waitForFinished(2000);
            say("Cancelled.");
        }
        if (go_) go_->setEnabled(true);
        if (cancel_) cancel_->setEnabled(false);
    }

    QDoubleSpinBox *start_, *end_, *by_;
    QComboBox *camera_, *format_;
    QSpinBox *width_, *height_;
    QLineEdit* dir_;
    QProgressBar* progress_;
    QPlainTextEdit* log_;
    QPushButton *go_ = nullptr, *cancel_ = nullptr;
    QList<double> frames_;
    int done_ = 0;
    QString file_;
    std::unique_ptr<QTemporaryDir> tmp_;
    QPointer<QProcess> proc_;
    QElapsedTimer clock_;
};

QString selected_shape(const QString& type) {
    for (const QJsonValue& s : ask("ls", QJsonObject{{"selection", true}, {"long", true}}).toArray()) {
        QString n = s.toString().section('.', 0, 0);
        QString t = ask("nodeType", one(n)).toString();
        if (t == type) return n;
        QJsonArray kids = ask("listRelatives", QJsonObject{{"args", QJsonArray{n}}, {"shapes", true}, {"fullPath", true}, {"type", type}}).toArray();
        if (!kids.isEmpty()) return kids.at(0).toString();
    }
    return QString();
}

} // namespace

void show_render_view(QWidget* parent) {
    view(parent);
}

void render_current_frame(QWidget* parent) {
    view(parent)->render_now();
}

void render_ipr(QWidget* parent) {
    view(parent)->start_ipr();
}

void redo_previous_render(QWidget* parent) {
    LastRender l = last_render();
    RenderView* v = view(parent);
    if (!l.any) v->render_now();
    else v->redo(l);
}

void render_sequence_dialog(QWidget* parent) {
    auto* d = new SequenceDialog(parent);
    d->show();
}

void create_harold_light(const QString& type) {
    exec("shadingNode", QJsonObject{{"args", QJsonArray{type}}, {"asLight", true}});
}

// A mesh light from the selected mesh: the light reads the mesh's shape through inMesh.
void create_mesh_light() {
    QString mesh = selected_shape("mesh");
    if (mesh.isEmpty()) {
        log("// Warning: select a mesh to make a mesh light from //", 4);
        return;
    }
    QString tr = exec("shadingNode", QJsonObject{{"args", QJsonArray{"aiMeshLight"}}, {"asLight", true}}).toString();
    if (tr.isEmpty()) return;
    QJsonArray shapes = ask("listRelatives", QJsonObject{{"args", QJsonArray{tr}}, {"shapes", true}, {"fullPath", true}}).toArray();
    QString light = shapes.isEmpty() ? tr : shapes.at(0).toString();
    exec("connectAttr", QJsonObject{{"args", QJsonArray{mesh + ".outMesh", light + ".inMesh"}}, {"force", true}});
    exec("select", one(tr));
}

// An atmosphere volume, made the scene's atmosphere.
void create_atmosphere() {
    QString n = exec("createNode", one("aiAtmosphereVolume")).toString();
    if (n.isEmpty()) return;
    exec("connectAttr", QJsonObject{{"args", QJsonArray{n + ".message", "defaultArnoldRenderOptions.atmosphere[0]"}}, {"force", true}});
    exec("select", one(n));
}

namespace {
void create_standard_surface() {
    bool any = !ask("ls", QJsonObject{{"selection", true}}).toArray().isEmpty();
    QString mat = exec("shadingNode", QJsonObject{{"args", QJsonArray{"standardSurface"}}, {"asShader", true}}).toString();
    if (mat.isEmpty()) return;
    if (any) exec("hyperShade", QJsonObject{{"assign", mat}});
    exec("select", one(mat));
}
} // namespace

void harold_create_menu(QMenu* create, QWidget*) {
    QMenu* m = create->addMenu("Harold");
    m->addAction(icon("skyDome", 16), "Sky Dome Light", [] { create_harold_light("aiSkyDomeLight"); });
    m->addAction(icon("haroldAreaLight", 16), "Area Light", [] { create_harold_light("aiAreaLight"); });
    m->addAction(icon("meshLight", 16), "Mesh Light", [] { create_mesh_light(); })->setStatusTip("Mesh Light: light from the selected mesh");
    m->addAction(icon("photometricLight", 16), "Photometric Light", [] { create_harold_light("aiPhotometricLight"); });
    m->addSeparator();
    m->addAction(icon("atmosphere", 16), "Atmosphere Volume", [] { create_atmosphere(); });
    m->addAction(icon("standardSurface", 16), "Standard Surface", [] { create_standard_surface(); })->setStatusTip("Standard Surface: a new material, assigned to the selection");
}

QMenu* render_menu(QWidget* parent) {
    auto* m = new QMenu("Render", parent);
    m->addAction(icon("renderFrame", 16), "Render Current Frame", [parent] { render_current_frame(parent); });
    m->addAction("Redo Previous Render", [parent] { redo_previous_render(parent); });
    m->addAction(icon("renderIpr", 16), "IPR Render Current Frame", [parent] { render_ipr(parent); });
    m->addSeparator();
    m->addAction("Render Sequence...", [parent] { render_sequence_dialog(parent); });
    m->addSeparator();
    m->addAction(icon("renderView", 16), "Harold RenderView", [parent] { show_render_view(parent); });
    m->addAction(icon("renderSettings", 16), "Render Settings...", [parent] { show_render_settings(parent); });
    return m;
}

QMenu* harold_menu(QWidget* parent) {
    auto* m = new QMenu("Harold", parent);
    m->addAction(icon("renderFrame", 16), "Render", [parent] { render_current_frame(parent); });
    m->addAction(icon("renderView", 16), "Open Harold RenderView", [parent] { show_render_view(parent); });
    m->addSeparator();
    QMenu* lights = m->addMenu("Lights");
    lights->addAction(icon("skyDome", 16), "Sky Dome Light", [] { create_harold_light("aiSkyDomeLight"); });
    lights->addAction(icon("haroldAreaLight", 16), "Area Light", [] { create_harold_light("aiAreaLight"); });
    lights->addAction(icon("meshLight", 16), "Mesh Light", [] { create_mesh_light(); });
    lights->addAction(icon("photometricLight", 16), "Photometric Light", [] { create_harold_light("aiPhotometricLight"); });
    m->addAction(icon("atmosphere", 16), "Atmosphere Volume", [] { create_atmosphere(); });
    m->addAction(icon("standardSurface", 16), "Standard Surface", [] { create_standard_surface(); });
    m->addSeparator();
    m->addAction(icon("renderSettings", 16), "Render Settings...", [parent] { show_render_settings(parent); });
    return m;
}

} // namespace az
