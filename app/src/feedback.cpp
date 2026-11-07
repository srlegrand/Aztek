// The report window: text, optional screenshot with drawing on top, optional zipped copy of the scene. Everything goes
// through the person's browser: see feedback.h. The screenshot stays in memory until they press Open on GitHub.
#include "feedback.h"

#include "aztek.h"
#include "host.h"
#include "viewport.h"

#include <QApplication>
#include <QCheckBox>
#include <QClipboard>
#include <QComboBox>
#include <QCursor>
#include <QDateTime>
#include <QDesktopServices>
#include <QDialog>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QHBoxLayout>
#include <QImage>
#include <QInputDialog>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLabel>
#include <QLineEdit>
#include <QMessageBox>
#include <QMouseEvent>
#include <QPainter>
#include <QPainterPath>
#include <QPlainTextEdit>
#include <QPointer>
#include <QProcess>
#include <QPushButton>
#include <QScreen>
#include <QSpinBox>
#include <QStandardPaths>
#include <QSysInfo>
#include <QTemporaryDir>
#include <QTimer>
#include <QToolButton>
#include <QUrl>
#include <QVBoxLayout>
#include <QtGlobal>

#include <algorithm>
#include <cmath>
#include <functional>
#include <vector>

#ifndef AZTEK_VERSION
#define AZTEK_VERSION "dev"
#endif

namespace az {

namespace {

const char* const kPublicWarning =
    "This will be posted to the public GitHub bug tracker, where anyone can see it. Never include work under NDA or anything "
    "confidential. If you do, it is at your own risk.";
const char* const kSceneWarning =
    "The scene file holds the names of everything in it and may hold file paths. Referenced files and textures are not included.";
const qint64 kFileLimit = 25 * 1024 * 1024; // GitHub's limit for attached files.
const qint64 kImageLimit = 10 * 1024 * 1024; // And for images.
const char* const kOwnWindow = "feedbackWindow";

// Whichever windows of the app are showing, viewports included, on one picture at their places on the screen. Report
// windows are left out. In memory only: a viewport's pixels pass through a temporary file that goes straight away.
QImage capture_app(QWidget* main_window) {
    QList<QWidget*> windows{main_window};
    for (QWidget* w : QApplication::topLevelWidgets()) {
        if (w == main_window || !w->isVisible() || w->isMinimized() || w->property(kOwnWindow).toBool()) continue;
        const Qt::WindowType type = w->windowType();
        if (type == Qt::Popup || type == Qt::ToolTip || type == Qt::SplashScreen) continue;
        windows << w;
    }
    // Wayland doesn't say where windows are, so they go one under another.
    const bool stacked = QGuiApplication::platformName().startsWith("wayland");
    QList<QPoint> at;
    QRect all;
    int below = 0;
    for (QWidget* w : windows) {
        QPoint p = stacked ? QPoint(0, below) : w->mapToGlobal(QPoint(0, 0));
        below += w->height();
        at << p;
        all |= QRect(p, w->size());
    }
    const qreal dpr = main_window->devicePixelRatioF();
    QImage out(all.size() * dpr, QImage::Format_RGB32);
    out.setDevicePixelRatio(dpr);
    out.fill(QColor(43, 43, 43));
    QPainter p(&out);
    QTemporaryDir tmp;
    for (int i = 0; i < windows.size(); ++i) {
        QWidget* w = windows[i];
        const QPoint origin = at[i] - all.topLeft();
        p.drawPixmap(origin, w->grab());
        for (ViewportWidget* v : w->findChildren<ViewportWidget*>()) {
            if (!v->isVisible() || !tmp.isValid()) continue;
            const QString file = tmp.filePath("view.ppm");
            if (!v->capture(file)) continue;
            p.drawImage(QRect(origin + v->mapTo(w, QPoint(0, 0)), v->size()), QImage(file));
            QFile::remove(file);
        }
    }
    p.end();
    out.setDevicePixelRatio(1.0);
    return out;
}

enum class Tool { Pen, Arrow, Rectangle, Text };

struct Mark {
    Tool tool = Tool::Pen;
    QColor colour;
    int width = 4;
    QVector<QPointF> points;
    QString text;
};

void draw_mark(QPainter& p, const Mark& m) {
    QPen pen(m.colour, m.width, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin);
    p.setPen(pen);
    p.setBrush(Qt::NoBrush);
    if (m.points.isEmpty()) return;
    const QPointF a = m.points.first(), b = m.points.last();
    switch (m.tool) {
    case Tool::Pen:
        if (m.points.size() == 1) p.drawPoint(a);
        else p.drawPolyline(m.points.constData(), m.points.size());
        break;
    case Tool::Rectangle:
        p.drawRect(QRectF(a, b).normalized());
        break;
    case Tool::Arrow: {
        p.drawLine(a, b);
        const QPointF d = b - a;
        const double len = std::hypot(d.x(), d.y());
        if (len < 1) break;
        const QPointF unit = d / len, side(-unit.y(), unit.x());
        const double head = std::min(len, 10.0 + m.width * 3.0);
        QPainterPath tip;
        tip.moveTo(b);
        tip.lineTo(b - unit * head + side * head * 0.5);
        tip.lineTo(b - unit * head - side * head * 0.5);
        tip.closeSubpath();
        p.fillPath(tip, m.colour);
        break;
    }
    case Tool::Text: {
        QFont f = p.font();
        f.setPixelSize(12 + m.width * 3);
        f.setBold(true);
        p.setFont(f);
        p.drawText(a, m.text);
        break;
    }
    }
}

// The picture and what is drawn on it, in image coordinates, kept apart so undo is simple and lines are sharp at full size.
class Annotation : public QObject {
    Q_OBJECT
public:
    QImage base;
    std::vector<Mark> marks;
    std::vector<std::vector<Mark>> undo_steps;
    Tool tool = Tool::Pen;
    QColor colour = QColor(230, 30, 30);
    int width = 4;

    void remember() { undo_steps.push_back(marks); }
    void undo() {
        if (undo_steps.empty()) return;
        marks = undo_steps.back();
        undo_steps.pop_back();
        emit changed();
    }
    void clear() {
        if (marks.empty()) return;
        remember();
        marks.clear();
        emit changed();
    }
    void set_picture(const QImage& image) {
        base = image;
        marks.clear();
        undo_steps.clear();
        emit changed();
    }
    QImage composite() const {
        QImage out = base.convertToFormat(QImage::Format_RGB32);
        QPainter p(&out);
        p.setRenderHint(QPainter::Antialiasing);
        for (const Mark& m : marks) draw_mark(p, m);
        return out;
    }
signals:
    void changed();
    void settings_changed();
};

// The picture scaled to fit, to draw on.
class Canvas : public QWidget {
public:
    explicit Canvas(Annotation* doc, QWidget* parent = nullptr) : QWidget(parent), doc_(doc) {
        setObjectName("feedbackCanvas");
        setMinimumSize(240, 150);
        setCursor(Qt::CrossCursor);
        setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
        connect(doc_, &Annotation::changed, this, qOverload<>(&QWidget::update));
    }

protected:
    void paintEvent(QPaintEvent*) override {
        QPainter p(this);
        p.fillRect(rect(), QColor(30, 30, 30));
        if (doc_->base.isNull()) return;
        p.setRenderHint(QPainter::SmoothPixmapTransform);
        p.setRenderHint(QPainter::Antialiasing);
        const QRectF r = picture_rect();
        p.drawImage(r, doc_->base);
        p.setClipRect(r);
        p.translate(r.topLeft());
        const double s = r.width() / doc_->base.width();
        p.scale(s, s);
        for (const Mark& m : doc_->marks) draw_mark(p, m);
        if (drawing_) draw_mark(p, current_);
    }
    void mousePressEvent(QMouseEvent* e) override {
        if (e->button() != Qt::LeftButton || doc_->base.isNull()) return;
        const QPointF at = to_image(e->position());
        if (doc_->tool == Tool::Text) {
            bool ok = false;
            const QString text = QInputDialog::getText(this, "Add Text", "Text:", QLineEdit::Normal, QString(), &ok);
            if (!ok || text.trimmed().isEmpty()) return;
            Mark m = fresh();
            m.points = {at};
            m.text = text;
            doc_->remember();
            doc_->marks.push_back(m);
            emit doc_->changed();
            return;
        }
        current_ = fresh();
        current_.points = {at};
        drawing_ = true;
        update();
    }
    void mouseMoveEvent(QMouseEvent* e) override {
        if (!drawing_) return;
        const QPointF at = to_image(e->position());
        if (current_.tool == Tool::Pen) current_.points << at;
        else current_.points = {current_.points.first(), at};
        update();
    }
    void mouseReleaseEvent(QMouseEvent* e) override {
        if (e->button() != Qt::LeftButton || !drawing_) return;
        drawing_ = false;
        doc_->remember();
        doc_->marks.push_back(current_);
        emit doc_->changed();
    }

private:
    Mark fresh() const {
        Mark m;
        m.tool = doc_->tool;
        m.colour = doc_->colour;
        m.width = doc_->width;
        return m;
    }
    QRectF picture_rect() const {
        QSizeF size = QSizeF(doc_->base.size()).scaled(QSizeF(this->size()), Qt::KeepAspectRatio);
        return QRectF(QPointF((width() - size.width()) / 2, (height() - size.height()) / 2), size);
    }
    QPointF to_image(QPointF p) const {
        const QRectF r = picture_rect();
        const double s = doc_->base.width() / r.width();
        return QPointF(qBound(0.0, (p.x() - r.x()) * s, double(doc_->base.width())), qBound(0.0, (p.y() - r.y()) * s, double(doc_->base.height())));
    }
    Annotation* doc_;
    Mark current_;
    bool drawing_ = false;
};

// Tools over a canvas. Two can share one annotation: the small one in the report and the large window.
class AnnotationView : public QWidget {
public:
    explicit AnnotationView(Annotation* doc, QWidget* parent = nullptr) : QWidget(parent), doc_(doc) {
        auto* v = new QVBoxLayout(this);
        v->setContentsMargins(0, 0, 0, 0);
        auto* row = new QHBoxLayout;
        row->setSpacing(2);
        const std::pair<const char*, Tool> tools[] = {{"Pen", Tool::Pen}, {"Arrow", Tool::Arrow}, {"Rectangle", Tool::Rectangle}, {"Text", Tool::Text}};
        for (const auto& [label, tool] : tools) {
            auto* b = new QToolButton(this);
            b->setText(label);
            b->setCheckable(true);
            b->setAutoExclusive(true);
            b->setChecked(doc_->tool == tool);
            Tool t = tool;
            connect(b, &QToolButton::clicked, this, [this, t] {
                doc_->tool = t;
                emit doc_->settings_changed();
            });
            tool_buttons_.push_back({b, t});
            row->addWidget(b);
        }
        colour_ = new QComboBox(this);
        const std::pair<const char*, QColor> colours[] = {{"Red", QColor(230, 30, 30)}, {"Yellow", QColor(255, 215, 0)}, {"Green", QColor(40, 200, 70)}, {"Blue", QColor(40, 130, 255)}, {"White", Qt::white}, {"Black", Qt::black}};
        for (const auto& [name, c] : colours) {
            QPixmap swatch(12, 12);
            swatch.fill(c);
            colour_->addItem(QIcon(swatch), name, c);
        }
        colour_->setCurrentIndex(colour_->findData(doc_->colour));
        connect(colour_, &QComboBox::activated, this, [this](int i) {
            doc_->colour = colour_->itemData(i).value<QColor>();
            emit doc_->settings_changed();
        });
        row->addWidget(colour_);
        width_ = new QSpinBox(this);
        width_->setRange(1, 30);
        width_->setValue(doc_->width);
        width_->setPrefix("Width ");
        connect(width_, qOverload<int>(&QSpinBox::valueChanged), this, [this](int w) {
            if (doc_->width == w) return;
            doc_->width = w;
            emit doc_->settings_changed();
        });
        row->addWidget(width_);
        auto* undo = new QToolButton(this);
        undo->setText("Undo");
        connect(undo, &QToolButton::clicked, doc_, &Annotation::undo);
        row->addWidget(undo);
        auto* clear = new QToolButton(this);
        clear->setText("Clear");
        connect(clear, &QToolButton::clicked, doc_, &Annotation::clear);
        row->addWidget(clear);
        row->addStretch(1);
        v->addLayout(row);
        v->addWidget(new Canvas(doc_, this), 1);
        // The other view's choices show here too.
        connect(doc_, &Annotation::settings_changed, this, [this] {
            for (const auto& [b, t] : tool_buttons_)
                if (t == doc_->tool) b->setChecked(true);
            colour_->setCurrentIndex(colour_->findData(doc_->colour));
            width_->setValue(doc_->width);
        });
    }

private:
    Annotation* doc_;
    std::vector<std::pair<QToolButton*, Tool>> tool_buttons_;
    QComboBox* colour_;
    QSpinBox* width_;
};

QString megabytes(qint64 bytes) {
    return QString::number(bytes / (1024.0 * 1024.0), 'f', 1) + " MB";
}

// Shows the folder, with the file selected where the system allows it.
void show_in_folder(const QString& folder, const QString& file) {
    const QString path = QDir(folder).filePath(file);
#if defined(Q_OS_WIN)
    if (!file.isEmpty() && QProcess::startDetached("explorer", {"/select,", QDir::toNativeSeparators(path)})) return;
#elif defined(Q_OS_LINUX)
    if (!file.isEmpty()) {
        QProcess p;
        p.start("dbus-send", {"--session", "--print-reply", "--dest=org.freedesktop.FileManager1", "/org/freedesktop/FileManager1",
                              "org.freedesktop.FileManager1.ShowItems", "array:string:" + QUrl::fromLocalFile(path).toString(), "string:"});
        if (p.waitForFinished(1500) && p.exitStatus() == QProcess::NormalExit && p.exitCode() == 0) return;
    }
#else
    Q_UNUSED(path);
#endif
    QDesktopServices::openUrl(QUrl::fromLocalFile(folder));
}

QString dry_run_file() {
    return qEnvironmentVariable("AZTEK_FEEDBACK_DRYRUN");
}

bool open_link(const QString& url) {
    if (!dry_run_file().isEmpty()) return true;
    return QDesktopServices::openUrl(QUrl::fromEncoded(url.toLatin1()));
}

// What is left to do on the page, after the browser has been asked to open it.
class NextSteps : public QDialog {
public:
    NextSteps(QWidget* parent, const QString& url, const QString& full_text, const QImage& picture, const QString& folder, const QString& file_to_show,
              const QString& picture_note, bool shortened, bool opened, bool has_zip) : QDialog(parent), picture_(picture) {
        setAttribute(Qt::WA_DeleteOnClose);
        setProperty(kOwnWindow, true);
        setObjectName("feedbackNextSteps");
        setWindowTitle("Next Steps");
        auto* v = new QVBoxLayout(this);

        QStringList steps;
        steps << (opened ? "GitHub has opened in your browser with your report filled in." : "<b>The browser did not open.</b> Press Copy Link and paste it into your browser.");
        steps << "Sign in if GitHub asks you to.";
        if (shortened) steps << "Your text was too long for a link and has been shortened. Press Copy Full Text and paste it over the shortened text.";
        if (!picture.isNull()) steps << "Click in the description box and press Ctrl+V to add the picture. It is on the clipboard. If that does nothing, drag the PNG in from the folder.";
        if (!picture_note.isEmpty()) steps << picture_note;
        if (has_zip) steps << "Drag <b>aztek-scene.zip</b> from the folder onto the description box.";
        steps << "Read it through, then press <b>Submit new issue</b>. Nothing is public until you do.";
        QString html = "<ol style=\"margin-left: 0px; -qt-list-indent: 1;\">";
        for (const QString& s : steps) html += "<li style=\"margin-bottom: 6px;\">" + s + "</li>";
        auto* list = new QLabel(html + "</ol>", this);
        list->setObjectName("feedbackSteps");
        list->setWordWrap(true);
        list->setTextFormat(Qt::RichText);
        v->addWidget(list);

        auto* row = new QHBoxLayout;
        auto add = [&](const QString& text, std::function<void()> fn) {
            auto* b = new QPushButton(text, this);
            b->setAutoDefault(false);
            connect(b, &QPushButton::clicked, this, fn);
            row->addWidget(b);
        };
        if (!picture.isNull()) add("Copy Screenshot Again", [this] { QApplication::clipboard()->setImage(picture_); });
        if (!folder.isEmpty()) add("Open Folder Again", [folder, file_to_show] {
            if (dry_run_file().isEmpty()) show_in_folder(folder, file_to_show);
        });
        add("Copy Full Text", [full_text] { QApplication::clipboard()->setText(full_text); });
        add("Copy Link", [url] { QApplication::clipboard()->setText(url); });
        row->addStretch(1);
        add("Close", [this] { close(); });
        v->addLayout(row);
        setMinimumWidth(560);
    }

private:
    QImage picture_;
};

class LargeDrawing : public QDialog {
public:
    LargeDrawing(QWidget* parent, Annotation* doc) : QDialog(parent) {
        setAttribute(Qt::WA_DeleteOnClose);
        setProperty(kOwnWindow, true);
        setObjectName("feedbackLargeDrawing");
        setWindowTitle("Draw on the Screenshot");
        auto* v = new QVBoxLayout(this);
        v->addWidget(new AnnotationView(doc, this), 1);
        auto* row = new QHBoxLayout;
        row->addStretch(1);
        auto* done = new QPushButton("Done", this);
        done->setAutoDefault(false);
        connect(done, &QPushButton::clicked, this, &QDialog::close);
        row->addWidget(done);
        v->addLayout(row);
        resize(QSize(1100, 760).boundedTo(parent->screen()->availableSize() * 0.9));
    }
};

class FeedbackDialog : public QDialog {
public:
    FeedbackDialog(QWidget* main_window, const QImage& picture) : QDialog(main_window), main_(main_window) {
        setAttribute(Qt::WA_DeleteOnClose);
        setProperty(kOwnWindow, true);
        setObjectName("feedbackDialog");
        setWindowTitle("This doesn't feel right!");
        doc_ = new Annotation;
        doc_->setParent(this);
        doc_->base = picture;

        auto* outer = new QHBoxLayout(this);
        auto* left = new QVBoxLayout;
        outer->addLayout(left, 1);

        auto* intro = new QLabel("Say what behaves differently from what you expect. This opens a GitHub issue page in your browser, "
                                 "filled in with what you write here. Nothing goes public until you press Submit on GitHub, and you need a GitHub account to do that.", this);
        intro->setWordWrap(true);
        left->addWidget(intro);

        title_ = new QLineEdit(this);
        title_->setObjectName("feedbackTitle");
        title_->setPlaceholderText("Short title (optional: the first line of the text is used otherwise)");
        left->addWidget(title_);
        text_ = new QPlainTextEdit(this);
        text_->setObjectName("feedbackText");
        text_->setPlaceholderText("What did you do? What did you expect? What happened?");
        text_->setTabChangesFocus(true);
        left->addWidget(text_, 2);

        auto* ticks = new QHBoxLayout;
        shot_tick_ = new QCheckBox("Include a screenshot", this);
        shot_tick_->setObjectName("feedbackScreenshot");
        scene_tick_ = new QCheckBox("Include my current scene file", this);
        scene_tick_->setObjectName("feedbackScene");
        ticks->addWidget(shot_tick_);
        ticks->addWidget(scene_tick_);
        ticks->addStretch(1);
        left->addLayout(ticks);

        warning_ = new QLabel(this);
        warning_->setObjectName("feedbackWarning");
        warning_->setWordWrap(true);
        warning_->hide();
        left->addWidget(warning_);

        left->addWidget(new QLabel("What the page will be filled in with:", this));
        preview_ = new QPlainTextEdit(this);
        preview_->setObjectName("feedbackPreview");
        preview_->setReadOnly(true);
        left->addWidget(preview_, 1);
        short_note_ = new QLabel("This is too long for a link, so the page opens with it shortened. Copy Full Text gets you the whole of it to paste in.", this);
        short_note_->setWordWrap(true);
        short_note_->hide();
        left->addWidget(short_note_);

        auto* buttons = new QHBoxLayout;
        copy_full_ = new QPushButton("Copy Full Text", this);
        copy_full_->setAutoDefault(false);
        copy_full_->hide();
        connect(copy_full_, &QPushButton::clicked, this, [this] { QApplication::clipboard()->setText(report_["fullBody"].toString()); });
        buttons->addWidget(copy_full_);
        buttons->addStretch(1);
        auto* cancel = new QPushButton("Cancel", this);
        cancel->setAutoDefault(false);
        connect(cancel, &QPushButton::clicked, this, &QDialog::reject);
        buttons->addWidget(cancel);
        submit_ = new QPushButton("Open on GitHub", this);
        submit_->setObjectName("feedbackSubmit");
        submit_->setAutoDefault(false);
        submit_->setToolTip("Opens the filled-in page in your browser. Nothing is posted until you press Submit there.");
        connect(submit_, &QPushButton::clicked, this, &FeedbackDialog::go);
        buttons->addWidget(submit_);
        left->addLayout(buttons);

        // The picture: only once the box is ticked and accepted.
        picture_box_ = new QWidget(this);
        auto* right = new QVBoxLayout(picture_box_);
        right->setContentsMargins(0, 0, 0, 0);
        auto* about_picture = new QLabel("Shows the main window and any editor windows that are open. Draw on it to point things out.", this);
        about_picture->setWordWrap(true);
        right->addWidget(about_picture);
        right->addWidget(new AnnotationView(doc_, picture_box_), 1);
        auto* picture_row = new QHBoxLayout;
        auto* larger = new QPushButton("Open Larger...", picture_box_);
        larger->setAutoDefault(false);
        connect(larger, &QPushButton::clicked, this, [this] {
            if (!large_) large_ = new LargeDrawing(this, doc_);
            large_->show();
            large_->raise();
        });
        auto* retake = new QPushButton("Retake", picture_box_);
        retake->setAutoDefault(false);
        retake->setToolTip("Hides this window and takes the screenshot again. What you have drawn is cleared.");
        connect(retake, &QPushButton::clicked, this, &FeedbackDialog::retake);
        picture_row->addWidget(larger);
        picture_row->addWidget(retake);
        picture_row->addStretch(1);
        right->addLayout(picture_row);
        picture_box_->hide();
        outer->addWidget(picture_box_, 2);

        connect(title_, &QLineEdit::textChanged, this, &FeedbackDialog::refresh);
        connect(text_, &QPlainTextEdit::textChanged, this, &FeedbackDialog::refresh);
        // clicked and not toggled: only the person's own click asks, and declining unticks without asking again.
        connect(shot_tick_, &QCheckBox::clicked, this, [this](bool on) { ticked(shot_tick_, on, false); });
        connect(scene_tick_, &QCheckBox::clicked, this, [this](bool on) { ticked(scene_tick_, on, true); });
        // The theme's placeholder colour is too dim to read here, and this text matters.
        QPalette hint = text_->palette();
        hint.setColor(QPalette::PlaceholderText, QColor(165, 165, 165));
        text_->setPalette(hint);
        title_->setPalette(hint);
        resize(620, 620);
        refresh();
    }

    void reject() override {
        if (!text_->toPlainText().trimmed().isEmpty()) {
            auto answer = QMessageBox::question(this, "Discard Report", "Discard what you have written?", QMessageBox::Discard | QMessageBox::Cancel, QMessageBox::Cancel);
            if (answer != QMessageBox::Discard) return;
        }
        QDialog::reject();
    }

private:
    void ticked(QCheckBox* box, bool on, bool scene) {
        if (on) {
            QMessageBox ask(this);
            ask.setIcon(QMessageBox::Warning);
            ask.setWindowTitle(scene ? "Include Your Scene File?" : "Include a Screenshot?");
            ask.setText(QString("<b>%1</b>").arg(kPublicWarning));
            if (scene) ask.setInformativeText(kSceneWarning);
            QPushButton* yes = ask.addButton("I Understand, Include It", QMessageBox::AcceptRole);
            QPushButton* no = ask.addButton("Don't Include It", QMessageBox::RejectRole);
            ask.setDefaultButton(no);
            ask.setEscapeButton(no);
            ask.exec();
            if (ask.clickedButton() != yes) box->setChecked(false);
        }
        const bool shot = shot_tick_->isChecked();
        if (shot != picture_box_->isVisible()) {
            if (shot) narrow_width_ = width();
            picture_box_->setVisible(shot);
            layout()->activate();
            resize(shot ? narrow_width_ + 560 : narrow_width_, height());
            keep_on_screen();
        }
        refresh();
    }

    void keep_on_screen() {
        const QRect room = screen()->availableGeometry();
        resize(qMin(width(), room.width()), qMin(height(), room.height()));
        move(qBound(room.left(), x(), qMax(room.left(), room.right() + 1 - width())), qBound(room.top(), y(), qMax(room.top(), room.bottom() + 1 - height())));
    }

    QJsonObject build_report(const QStringList& attachments) const {
        QJsonObject call{{"title", title_->text()},
                         {"description", text_->toPlainText()},
                         {"version", AZTEK_VERSION},
                         {"os", QSysInfo::prettyProductName()},
                         {"qt", QString(qVersion())},
                         {"attachments", QJsonArray::fromStringList(attachments)}};
        const QByteArray json = QJsonDocument(call).toJson(QJsonDocument::Compact);
        return parse(take(aztek_feedback_issue(json.constData()))).toObject();
    }

    QStringList ticked_attachments() const {
        QStringList out;
        if (shot_tick_->isChecked()) out << "screenshot";
        if (scene_tick_->isChecked()) out << "scene file (zip)";
        return out;
    }

    void refresh() {
        const bool has_text = !text_->toPlainText().trimmed().isEmpty();
        submit_->setEnabled(has_text);
        report_ = build_report(ticked_attachments());
        const bool shortened = report_["shortened"].toBool();
        preview_->setPlainText(has_text ? "Title: " + report_["title"].toString() + "\n\n" + report_["body"].toString() : QString());
        short_note_->setVisible(shortened);
        copy_full_->setVisible(shortened);
        const bool warn = shot_tick_->isChecked() || scene_tick_->isChecked();
        if (warn) {
            QString text = kPublicWarning;
            if (scene_tick_->isChecked()) text += QString(" ") + kSceneWarning;
            warning_->setText(text);
        }
        warning_->setVisible(warn);
    }

    void retake() {
        if (!doc_->marks.empty()) {
            auto answer = QMessageBox::question(this, "Retake Screenshot", "Taking it again clears what you have drawn. Go on?", QMessageBox::Yes | QMessageBox::Cancel, QMessageBox::Cancel);
            if (answer != QMessageBox::Yes) return;
        }
        if (large_) large_->close();
        hide();
        // Long enough for the window system to take this window off the screen and repaint what was under it.
        QTimer::singleShot(250, this, [this] {
            QApplication::processEvents();
            doc_->set_picture(capture_app(main_));
            show();
            raise();
            activateWindow();
        });
    }

    // Writes what was ticked, opens the browser and shows what is left to do.
    void go() {
        const bool want_shot = shot_tick_->isChecked(), want_scene = scene_tick_->isChecked();
        QString folder;
        if (want_shot || want_scene) {
            folder = QDir(QStandardPaths::writableLocation(QStandardPaths::AppDataLocation)).filePath("feedback/" + QDateTime::currentDateTime().toString("yyyyMMdd-HHmmss"));
            if (!QDir().mkpath(folder)) {
                QMessageBox::warning(this, "Can't Save", "Could not make the folder for the attachments:\n" + folder);
                return;
            }
        }
        QString zip_name;
        if (want_scene) {
            const QString zip_path = QDir(folder).filePath("aztek-scene.zip");
            QApplication::setOverrideCursor(Qt::WaitCursor);
            const QJsonObject r = parse(take(aztek_feedback_scene_zip(zip_path.toUtf8().constData()))).toObject();
            QApplication::restoreOverrideCursor();
            if (r.contains("error")) {
                QMessageBox::warning(this, "Can't Save the Scene", "The scene copy could not be written:\n" + r["error"].toString());
                return;
            }
            const qint64 bytes = qint64(r["ok"].toDouble());
            if (bytes > kFileLimit) {
                QMessageBox big(this);
                big.setIcon(QMessageBox::Warning);
                big.setWindowTitle("Scene File Too Large");
                big.setText(QString("The zipped scene is %1. GitHub takes attachments up to 25 MB, so it can't be attached.").arg(megabytes(bytes)));
                QPushButton* on = big.addButton("Go On Without the Scene File", QMessageBox::AcceptRole);
                QPushButton* back = big.addButton("Go Back", QMessageBox::RejectRole);
                big.setDefaultButton(back);
                big.setEscapeButton(back);
                big.exec();
                QFile::remove(zip_path);
                if (big.clickedButton() != on) {
                    if (!want_shot) QDir().rmdir(folder);
                    return;
                }
                if (!want_shot) {
                    QDir().rmdir(folder);
                    folder.clear();
                }
            } else {
                zip_name = "aztek-scene.zip";
            }
        }
        QImage picture;
        QString picture_note, png_name;
        if (want_shot) {
            picture = doc_->composite();
            png_name = "aztek-screenshot.png";
            const QString png_path = QDir(folder).filePath(png_name);
            if (!picture.save(png_path, "PNG")) {
                QMessageBox::warning(this, "Can't Save the Screenshot", "The screenshot could not be written:\n" + png_path);
                return;
            }
            const qint64 bytes = QFileInfo(png_path).size();
            if (bytes > kImageLimit) picture_note = QString("The picture is %1. GitHub takes images up to 10 MB, so it may be refused: retake it with a smaller window if so.").arg(megabytes(bytes));
            QApplication::clipboard()->setImage(picture);
        }

        QStringList attachments;
        if (want_shot) attachments << "screenshot";
        if (!zip_name.isEmpty()) attachments << "scene file (zip)";
        const QJsonObject report = build_report(attachments);
        const QString url = report["url"].toString();
        const QString show_file = zip_name.isEmpty() ? png_name : zip_name;

        const QString dry = dry_run_file();
        if (!dry.isEmpty()) {
            QFile f(dry);
            if (f.open(QIODevice::WriteOnly | QIODevice::Text)) {
                f.write(url.toLatin1() + "\n");
                f.write("folder=" + folder.toUtf8() + "\n");
                f.write("show=" + show_file.toUtf8() + "\n");
            }
        } else if (!folder.isEmpty()) {
            show_in_folder(folder, show_file);
        }
        const bool opened = open_link(url);
        auto* steps = new NextSteps(main_, url, report["fullBody"].toString(), picture, folder, show_file, picture_note, report["shortened"].toBool(), opened, !zip_name.isEmpty());
        steps->show();
        QDialog::accept();
    }

    QWidget* main_;
    Annotation* doc_;
    QLineEdit* title_;
    QPlainTextEdit* text_;
    QPlainTextEdit* preview_;
    QCheckBox* shot_tick_;
    QCheckBox* scene_tick_;
    QLabel* warning_;
    QLabel* short_note_;
    QPushButton* copy_full_;
    QPushButton* submit_;
    QWidget* picture_box_;
    QPointer<LargeDrawing> large_;
    int narrow_width_ = 620;
    QJsonObject report_;
};

QPointer<FeedbackDialog>& open_dialog() {
    static QPointer<FeedbackDialog> d;
    return d;
}

} // namespace

QWidget* make_feedback_button(QWidget* parent) {
    auto* b = new QPushButton("This doesn't feel right!", parent);
    b->setObjectName("feelButton");
    b->setFocusPolicy(Qt::NoFocus);
    b->setCursor(Qt::PointingHandCursor);
    b->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Fixed);
    b->setToolTip("Something behaves differently from what you expect? Tell us.\nWrite a report and open it as a GitHub issue. Nothing is public until you press Submit on GitHub.");
    QObject::connect(b, &QPushButton::clicked, b, [b] { open_feedback(b->window()); });
    return b;
}

void open_feedback(QWidget* main_window) {
    if (FeedbackDialog* d = open_dialog()) {
        d->show();
        d->raise();
        d->activateWindow();
        return;
    }
    // Before the window shows, so it isn't in the picture. Thrown away unless the box gets ticked.
    auto* d = new FeedbackDialog(main_window, capture_app(main_window));
    open_dialog() = d;
    d->show();
    d->raise();
    d->activateWindow();
}

} // namespace az

#include "feedback.moc"
