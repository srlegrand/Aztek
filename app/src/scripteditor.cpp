// The Script Editor: the history pane over tabs of MEL and Python, with syntax colours, line numbers, Execute and the
// usual file and options menus.
#include "panels.h"
#include "host.h"
#include "icons.h"

#include <QAction>
#include <QApplication>
#include <QCheckBox>
#include <QDrag>
#include <QFile>
#include <QFileDialog>
#include <QFileInfo>
#include <QInputDialog>
#include <QJsonArray>
#include <QKeyEvent>
#include <QLineEdit>
#include <QMenu>
#include <QMenuBar>
#include <QMimeData>
#include <QPainter>
#include <QPlainTextEdit>
#include <QRegularExpression>
#include <QScrollBar>
#include <QSettings>
#include <QSplitter>
#include <QSyntaxHighlighter>
#include <QTabBar>
#include <QTabWidget>
#include <QTextBlock>
#include <QToolBar>
#include <QVBoxLayout>

namespace az {

namespace {

class Highlighter : public QSyntaxHighlighter {
public:
    Highlighter(QTextDocument* d, bool py) : QSyntaxHighlighter(d), python_(py) {
        QTextCharFormat kw, str, com, num, var, flag;
        kw.setForeground(QColor(120, 170, 240));
        kw.setFontWeight(QFont::Bold);
        str.setForeground(QColor(230, 190, 110));
        com.setForeground(QColor(120, 170, 120));
        num.setForeground(QColor(190, 150, 230));
        var.setForeground(QColor(120, 215, 215));
        flag.setForeground(QColor(170, 200, 130));
        QString kws = py ? "and|as|assert|break|class|continue|def|del|elif|else|except|finally|for|from|global|if|import|in|is|lambda|None|nonlocal|not|or|pass|raise|return|True|False|try|while|with|yield"
                         : "if|else|for|in|while|do|switch|case|default|break|continue|return|proc|global|int|float|string|vector|matrix|source";
        rules_.append({QRegularExpression("\\b(" + kws + ")\\b"), kw});
        rules_.append({QRegularExpression("\\b[0-9]+(\\.[0-9]*)?\\b"), num});
        if (!py) {
            rules_.append({QRegularExpression("\\$[A-Za-z_][A-Za-z0-9_]*"), var});
            rules_.append({QRegularExpression("(?<=\\s)-[A-Za-z][A-Za-z0-9]*"), flag});
        }
        rules_.append({QRegularExpression("\"(\\\\.|[^\"\\\\])*\"|'(\\\\.|[^'\\\\])*'"), str});
        rules_.append({QRegularExpression(py ? "#[^\n]*" : "//[^\n]*"), com});
    }
protected:
    void highlightBlock(const QString& text) override {
        for (const auto& r : rules_) {
            auto it = r.first.globalMatch(text);
            while (it.hasNext()) {
                auto m = it.next();
                setFormat(m.capturedStart(), m.capturedLength(), r.second);
            }
        }
    }
private:
    bool python_;
    QList<QPair<QRegularExpression, QTextCharFormat>> rules_;
};

class Gutter;

// A script tab: line numbers in a gutter, and a middle drag of the selected text to a shelf.
class ScriptEdit : public QPlainTextEdit {
public:
    ScriptEdit(const QString& language, QWidget* parent);
    QString language;
    QString path;
    void set_numbers(bool on) {
        numbers_ = on;
        update_margin();
    }
    int gutter_width() const {
        if (!numbers_) return 0;
        int digits = QString::number(qMax(1, blockCount())).size();
        return 8 + fontMetrics().horizontalAdvance('9') * qMax(3, digits);
    }
    void paint_gutter(QPaintEvent* e);
protected:
    void resizeEvent(QResizeEvent* e) override;
    void mousePressEvent(QMouseEvent* e) override;
private:
    void update_margin() { setViewportMargins(gutter_width(), 0, 0, 0); }
    bool numbers_ = false;
    QWidget* gutter_;
    friend class Gutter;
};

class Gutter : public QWidget {
public:
    explicit Gutter(ScriptEdit* e) : QWidget(e), edit_(e) {}
protected:
    void paintEvent(QPaintEvent* e) override { edit_->paint_gutter(e); }
private:
    ScriptEdit* edit_;
};

ScriptEdit::ScriptEdit(const QString& lang, QWidget* parent) : QPlainTextEdit(parent), language(lang) {
    gutter_ = new Gutter(this);
    connect(this, &QPlainTextEdit::blockCountChanged, this, [this] { update_margin(); });
    connect(this, &QPlainTextEdit::updateRequest, this, [this](const QRect& r, int dy) {
        if (dy) gutter_->scroll(0, dy);
        else gutter_->update(0, r.y(), gutter_->width(), r.height());
    });
    new Highlighter(document(), lang == "python");
    setObjectName("scriptInput");
    setLineWrapMode(QPlainTextEdit::NoWrap);
}

void ScriptEdit::resizeEvent(QResizeEvent* e) {
    QPlainTextEdit::resizeEvent(e);
    QRect c = contentsRect();
    gutter_->setGeometry(QRect(c.left(), c.top(), gutter_width(), c.height()));
}

void ScriptEdit::paint_gutter(QPaintEvent* e) {
    QPainter p(gutter_);
    p.fillRect(e->rect(), QColor(50, 50, 50));
    QTextBlock b = firstVisibleBlock();
    int n = b.blockNumber();
    int top = qRound(blockBoundingGeometry(b).translated(contentOffset()).top());
    p.setPen(QColor(150, 150, 150));
    while (b.isValid() && top <= e->rect().bottom()) {
        p.drawText(0, top, gutter_->width() - 4, fontMetrics().height(), Qt::AlignRight, QString::number(n + 1));
        b = b.next();
        top += qRound(blockBoundingRect(b).height());
        ++n;
    }
}

void ScriptEdit::mousePressEvent(QMouseEvent* e) {
    if (e->button() == Qt::MiddleButton && textCursor().hasSelection()) {
        auto* drag = new QDrag(this);
        auto* m = new QMimeData;
        m->setText(textCursor().selectedText().replace(QChar(0x2029), '\n'));
        m->setData("application/x-aztek-script-language", language.toUtf8());
        drag->setMimeData(m);
        drag->exec(Qt::CopyAction);
        return;
    }
    QPlainTextEdit::mousePressEvent(e);
}

ScriptEdit* edit_at(QTabWidget* tabs, int i) {
    return dynamic_cast<ScriptEdit*>(tabs->widget(i));
}

} // namespace

ScriptEditor::ScriptEditor(QWidget* parent) : QWidget(parent) {
    auto* lay = new QVBoxLayout(this);
    lay->setContentsMargins(0, 0, 0, 0);
    lay->setSpacing(0);
    QSettings s;
    show_numbers_ = s.value("scriptEditor/lineNumbers", true).toBool();
    stack_trace_ = s.value("scriptEditor/stackTrace", true).toBool();

    auto* mb = new QMenuBar(this);
    mb->setNativeMenuBar(false);
    QMenu* file = mb->addMenu("File");
    file->addAction("Load Script...", [this] {
        QString f = QFileDialog::getOpenFileName(this, "Load Script", QSettings().value("project/current").toString(), "Scripts (*.mel *.py);;All files (*)");
        if (!f.isEmpty()) load_file(f);
    });
    file->addAction("Source Script...", [] {
        QString f = QFileDialog::getOpenFileName(nullptr, "Source Script", QSettings().value("project/current").toString(), "Scripts (*.mel *.py)");
        if (f.isEmpty()) return;
        QFile in(f);
        if (!in.open(QIODevice::ReadOnly)) return;
        if (f.endsWith(".py", Qt::CaseInsensitive)) python(QString::fromUtf8(in.readAll()));
        else mel(QString::fromUtf8(in.readAll()));
    });
    file->addAction("Save Script...", [this] { save_current(false); });
    file->addAction("Save Script As...", [this] { save_current(true); });
    QMenu* edit = mb->addMenu("Edit");
    edit->addAction("Clear History", [this] { history_->clear(); });
    edit->addAction("Clear Input", [this] { if (auto* e = current()) e->clear(); });
    edit->addAction("Clear All", [this] { history_->clear(); if (auto* e = current()) e->clear(); });
    QMenu* cmd = mb->addMenu("Command");
    cmd->addAction("New Tab: MEL", [this] { add_tab("mel", "MEL", QString()); });
    cmd->addAction("New Tab: Python", [this] { add_tab("python", "Python", "import maya.cmds as cmds\n"); });
    cmd->addAction("Rename Tab...", [this] { rename_tab(inputs_->currentIndex()); });
    cmd->addAction("Close Tab", [this] { close_tab(inputs_->currentIndex()); });
    cmd->addSeparator();
    cmd->addAction("Execute\tCtrl+Enter", [this] { run(false); });
    cmd->addAction("Execute and Clear\tNumpad Enter", [this] { run(true); });
    QMenu* hist = mb->addMenu("History");
    QAction* echo = hist->addAction("Echo All Commands");
    echo->setCheckable(true);
    echo->setChecked(s.value("scriptEditor/echoAll", false).toBool());
    echo_all_ = echo->isChecked();
    connect(echo, &QAction::toggled, this, [this](bool on) {
        echo_all_ = on;
        QSettings().setValue("scriptEditor/echoAll", on);
        exec_quiet("uiPref", QJsonObject{{"args", QJsonArray{"echoAllCommands", on}}});
    });
    QAction* trace = hist->addAction("Show Stack Trace");
    trace->setCheckable(true);
    trace->setChecked(stack_trace_);
    connect(trace, &QAction::toggled, this, [this](bool on) {
        stack_trace_ = on;
        QSettings().setValue("scriptEditor/stackTrace", on);
    });
    QAction* numbers = hist->addAction("Show Line Numbers");
    numbers->setCheckable(true);
    numbers->setChecked(show_numbers_);
    connect(numbers, &QAction::toggled, this, [this](bool on) {
        show_numbers_ = on;
        QSettings().setValue("scriptEditor/lineNumbers", on);
        for (int i = 0; i < inputs_->count(); ++i)
            if (ScriptEdit* e = edit_at(inputs_, i)) e->set_numbers(on);
    });
    lay->addWidget(mb);

    auto* bar = new QToolBar(this);
    bar->setIconSize(QSize(18, 18));
    bar->addAction(icon("open"), "Load Script", [this, file] { file->actions().at(0)->trigger(); });
    bar->addAction(icon("save"), "Save Script", [this] { save_current(false); });
    bar->addSeparator();
    bar->addAction(icon("clearHistory"), "Clear History", [this] { history_->clear(); });
    bar->addAction(icon("clearInput"), "Clear Input", [this] { if (auto* e = current()) e->clear(); });
    bar->addSeparator();
    bar->addAction(icon("play"), "Execute (Ctrl+Enter): the selection, or the whole tab", [this] { run(false); });
    lay->addWidget(bar);
    auto* split = new QSplitter(Qt::Vertical, this);
    history_ = new QPlainTextEdit(split);
    history_->setReadOnly(true);
    history_->setObjectName("scriptHistory");
    history_->setMaximumBlockCount(20000);
    inputs_ = new QTabWidget(split);
    inputs_->setDocumentMode(true);
    inputs_->setTabsClosable(true);
    inputs_->setMovable(true);
    connect(inputs_, &QTabWidget::tabCloseRequested, this, &ScriptEditor::close_tab);
    inputs_->tabBar()->setContextMenuPolicy(Qt::CustomContextMenu);
    connect(inputs_->tabBar(), &QWidget::customContextMenuRequested, this, [this](const QPoint& p) {
        int i = inputs_->tabBar()->tabAt(p);
        QMenu m;
        m.addAction("New Tab: MEL", [this] { add_tab("mel", "MEL", QString()); });
        m.addAction("New Tab: Python", [this] { add_tab("python", "Python", "import maya.cmds as cmds\n"); });
        if (i >= 0) {
            m.addAction("Rename...", [this, i] { rename_tab(i); });
            m.addAction("Close", [this, i] { close_tab(i); });
        }
        m.exec(inputs_->tabBar()->mapToGlobal(p));
    });
    connect(inputs_->tabBar(), &QTabBar::tabBarDoubleClicked, this, [this](int i) { if (i >= 0) rename_tab(i); });
    split->setStretchFactor(0, 3);
    split->setStretchFactor(1, 2);
    lay->addWidget(split, 1);
    QFont mono("monospace");
    mono.setStyleHint(QFont::Monospace);
    mono.setPointSize(9);
    history_->setFont(mono);
    // The tabs left open last time, with their text.
    int n = s.beginReadArray("scriptEditor/tabs");
    for (int i = 0; i < n; ++i) {
        s.setArrayIndex(i);
        add_tab(s.value("language").toString(), s.value("name").toString(), s.value("text").toString(), s.value("path").toString());
    }
    s.endArray();
    if (inputs_->count() == 0) {
        add_tab("mel", "MEL", QString());
        add_tab("python", "Python", "import maya.cmds as cmds\n");
    }
    inputs_->setCurrentIndex(0);
}

ScriptEditor::~ScriptEditor() {
    store_tabs();
}

void ScriptEditor::load_file(const QString& f) {
    QFile in(f);
    if (!in.open(QIODevice::ReadOnly)) return;
    add_tab(f.endsWith(".py", Qt::CaseInsensitive) ? "python" : "mel", QFileInfo(f).fileName(), QString::fromUtf8(in.readAll()), f);
}

void ScriptEditor::store_tabs() {
    QSettings s;
    s.beginWriteArray("scriptEditor/tabs");
    for (int i = 0; i < inputs_->count(); ++i) {
        ScriptEdit* e = edit_at(inputs_, i);
        if (!e) continue;
        s.setArrayIndex(i);
        s.setValue("language", e->language);
        s.setValue("name", inputs_->tabText(i));
        s.setValue("text", e->toPlainText());
        s.setValue("path", e->path);
    }
    s.endArray();
}

void ScriptEditor::add_tab(const QString& language, const QString& name, const QString& text, const QString& path) {
    auto* e = new ScriptEdit(language == "python" ? "python" : "mel", inputs_);
    QFont mono("monospace");
    mono.setStyleHint(QFont::Monospace);
    mono.setPointSize(9);
    e->setFont(mono);
    e->setTabStopDistance(4 * e->fontMetrics().horizontalAdvance(' '));
    e->installEventFilter(this);
    e->set_numbers(show_numbers_);
    e->setPlainText(text);
    e->path = path;
    inputs_->setCurrentIndex(inputs_->addTab(e, name));
}

void ScriptEditor::rename_tab(int i) {
    if (i < 0) return;
    bool ok = false;
    QString n = QInputDialog::getText(this, "Rename Tab", "Name:", QLineEdit::Normal, inputs_->tabText(i), &ok);
    if (ok && !n.trimmed().isEmpty()) inputs_->setTabText(i, n.trimmed());
}

void ScriptEditor::close_tab(int i) {
    if (i < 0 || inputs_->count() < 2) return;
    QWidget* w = inputs_->widget(i);
    inputs_->removeTab(i);
    w->deleteLater();
}

QPlainTextEdit* ScriptEditor::current() const {
    return qobject_cast<QPlainTextEdit*>(inputs_->currentWidget());
}

void ScriptEditor::save_current(bool as) {
    ScriptEdit* e = edit_at(inputs_, inputs_->currentIndex());
    if (!e) return;
    QString f = e->path;
    if (as || f.isEmpty()) {
        QString filter = e->language == "python" ? "Python (*.py)" : "MEL (*.mel)";
        f = QFileDialog::getSaveFileName(this, "Save Script", QSettings().value("project/current").toString(), filter);
        if (f.isEmpty()) return;
    }
    QFile out(f);
    if (!out.open(QIODevice::WriteOnly)) {
        log("// Error: can't write " + f + " //", 2);
        return;
    }
    out.write(e->toPlainText().toUtf8());
    e->path = f;
    inputs_->setTabText(inputs_->currentIndex(), QFileInfo(f).fileName());
}

void ScriptEditor::append(const QJsonArray& lines) {
    static const char* colors[] = {"#d2d2d2", "#9fd0ff", "#ff8080", "#a0a0a0", "#f0c060"};
    for (const QJsonValue& v : lines) {
        QJsonObject l = v.toObject();
        int k = qBound(0, l["kind"].toInt(), 4);
        QString text = l["text"].toString();
        // Python's traceback lines are errors with file lines; Show Stack Trace off keeps only the last.
        if (!stack_trace_ && k == 2 && (text.startsWith("Traceback") || text.startsWith("  File "))) continue;
        history_->appendHtml(QString("<span style='color:%1; white-space:pre'>%2</span>").arg(colors[k], text.toHtmlEscaped()));
    }
    history_->verticalScrollBar()->setValue(history_->verticalScrollBar()->maximum());
}

void ScriptEditor::run(bool clear_after) {
    ScriptEdit* e = edit_at(inputs_, inputs_->currentIndex());
    if (!e) return;
    QString src = e->textCursor().hasSelection() ? e->textCursor().selectedText().replace(QChar(0x2029), '\n') : e->toPlainText();
    if (e->language == "python")
        python(src);
    else
        mel(src);
    if (clear_after) e->clear();
    store_tabs();
}

bool ScriptEditor::eventFilter(QObject* o, QEvent* ev) {
    if (ev->type() == QEvent::KeyPress) {
        auto* k = static_cast<QKeyEvent*>(ev);
        bool enter = k->key() == Qt::Key_Return || k->key() == Qt::Key_Enter;
        // Ctrl+Enter runs and keeps the text; keypad Enter runs and clears it.
        if (enter && (k->modifiers() & Qt::ControlModifier)) {
            run(false);
            return true;
        }
        if (k->key() == Qt::Key_Enter && (k->modifiers() & Qt::KeypadModifier)) {
            run(true);
            return true;
        }
    }
    return QWidget::eventFilter(o, ev);
}

} // namespace az
