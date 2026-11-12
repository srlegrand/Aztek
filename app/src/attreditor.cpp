#include <QJsonDocument>
// The Attribute Editor: a tab per node in the chain, sections by node type, sliders on ranged attributes, a Notes box, and
// Select, Load Attributes and Copy Tab along the bottom.
#include "panels.h"
#include "colorchooser.h"
#include "host.h"
#include "icons.h"
#include "rendernodes.h"

#include <QApplication>
#include <QCheckBox>
#include <QColorDialog>
#include <QFileDialog>
#include <QFileInfo>
#include <QMenu>
#include <QComboBox>
#include <QFormLayout>
#include <QHash>
#include <QHBoxLayout>
#include <QJsonArray>
#include <QLabel>
#include <QLineEdit>
#include <QPlainTextEdit>
#include <QPointer>
#include <QPushButton>
#include <QScrollArea>
#include <QMouseEvent>
#include <QPainter>
#include <QSlider>
#include <QStyle>
#include <QWheelEvent>
#include <QTabBar>
#include <QTabWidget>
#include <QToolButton>
#include <QVBoxLayout>
#include <cmath>

namespace az {

namespace {

QJsonObject args(const QJsonArray& a) {
    QJsonObject o;
    o["args"] = a;
    return o;
}

QString fmt3(double v) {
    QString s = QString::number(v, 'f', 3);
    return s == "-0.000" ? "0.000" : s;
}

void set_plug(const QString& plug, const QJsonValue& v, const QString& type) {
    QJsonObject call = args(QJsonArray{plug, v});
    if (type == "string") call["type"] = "string";
    exec("setAttr", call);
}

// Which section an attribute falls in, by node type, in the order sections come. Anything else goes to Extra Attributes.
struct Section { const char* title; const char* attrs; };

// In the order the Attribute Editor lists them under Display.
const char* const kDrawingOverrides = "overrideEnabled overrideDisplayType overrideLevelOfDetail overrideShading overrideTexturing overridePlayback overrideVisibility hideOnPlayback overrideRGBColors overrideColor overrideColorRGB";

QString override_label(const QString& attr, const QString& nice) {
    static const QHash<QString, QString> labels{{"overrideEnabled", "Enable Overrides"},  {"overrideDisplayType", "Display Type"}, {"overrideLevelOfDetail", "Level of Detail"},
                                                {"overrideShading", "Shading"},           {"overrideTexturing", "Texturing"},      {"overridePlayback", "Playback"},
                                                {"overrideVisibility", "Visible"},        {"hideOnPlayback", "Hide on Playback"},  {"overrideRGBColors", "Enable RGB Colors"},
                                                {"overrideColor", "Color"},               {"overrideColorRGB", "Color RGB"}};
    if (labels.contains(attr)) return labels.value(attr);
    // Harold's attributes keep the ai prefix of the files they come from; the frames they sit in say whose they are.
    static const QHash<QString, QString> harold{{"aiSubdivType", "Type"}, {"aiSubdivIterations", "Iterations"}, {"aiSubdivAdaptiveMetric", "Adaptive Metric"},
                                                {"aiSubdivPixelError", "Adaptive Error"}, {"aiSubdivUvSmoothing", "UV Smoothing"}, {"aiSubdivSmoothDerivs", "Smooth Tangents"},
                                                {"aiDispHeight", "Height"}, {"aiDispZeroValue", "Scalar Zero Value"}, {"aiDispPadding", "Bounds Padding"}, {"aiDispAutobump", "Auto Bump"},
                                                {"aiVisibleInDiffuseReflection", "Diffuse Reflection"}, {"aiVisibleInSpecularReflection", "Specular Reflection"},
                                                {"aiVisibleInDiffuseTransmission", "Diffuse Transmission"}, {"aiVisibleInSpecularTransmission", "Specular Transmission"},
                                                {"aiVisibleInVolume", "Volume"}, {"aiSss", "SSS"}, {"aiAov", "AOV Light Group"}, {"aiSssSetname", "SSS Set Name"},
                                                {"aiCastVolumetricShadows", "Volumetric Shadows"}, {"aiTranslator", "Light Shape"}, {"aiFilename", "Photometry File"}, {"fStop", "F Stop"}};
    if (harold.contains(attr)) return harold.value(attr);
    if (attr.startsWith("ai") && attr.size() > 2 && attr[2].isUpper() && nice.startsWith("Ai ")) return nice.mid(3);
    return nice;
}

// Harold's frames on lights, shapes and cameras.
const char* const kHaroldLight = "aiExposure aiSamples aiRadius aiAngle aiSpread aiRoundness aiSoftEdge aiResolution aiNormalize aiCastShadows aiShadowDensity aiCastVolumetricShadows aiDiffuse aiSpecular aiSss aiIndirect aiVolume aiTransmission aiMaxBounces aiVolumeSamples aiDecayType aiAov";
const char* const kRenderStats = "castsShadows receiveShadows primaryVisibility visibleInReflections visibleInRefractions doubleSided smoothShading motionBlur";
const char* const kHaroldShape = "aiOpaque aiMatte aiSelfShadows aiVisibleInDiffuseReflection aiVisibleInSpecularReflection aiVisibleInDiffuseTransmission aiVisibleInSpecularTransmission aiVisibleInVolume aiTraceSets aiSssSetname";
const char* const kHaroldSubdiv = "aiSubdivType aiSubdivIterations aiSubdivAdaptiveMetric aiSubdivPixelError aiSubdivUvSmoothing aiSubdivSmoothDerivs";
const char* const kHaroldDisp = "aiDispHeight aiDispZeroValue aiDispPadding aiDispAutobump";

// The standard surface's groups, as its specification orders them; snake case for the file name aiStandardSurface.
QList<Section> standard_sections(bool snake) {
    static QList<QByteArray> keep;
    auto s = [snake](const char* camel, const char* sn) {
        keep.append(QByteArray(snake ? sn : camel));
        return keep.last().constData();
    };
    return {
        {"Base", s("base baseColor diffuseRoughness metalness", "base base_color diffuse_roughness metalness")},
        {"Specular", s("specular specularColor specularRoughness specularIOR specularAnisotropy specularRotation", "specular specular_color specular_roughness specular_IOR specular_anisotropy specular_rotation")},
        {"Transmission", s("transmission transmissionColor transmissionDepth transmissionScatter transmissionScatterAnisotropy transmissionDispersion transmissionExtraRoughness",
                           "transmission transmission_color transmission_depth transmission_scatter transmission_scatter_anisotropy transmission_dispersion transmission_extra_roughness")},
        {"Subsurface", s("subsurface subsurfaceColor subsurfaceRadius subsurfaceScale subsurfaceAnisotropy subsurfaceType", "subsurface subsurface_color subsurface_radius subsurface_scale subsurface_anisotropy subsurface_type")},
        {"Coat", s("coat coatColor coatRoughness coatAnisotropy coatRotation coatIOR coatNormal coatAffectColor coatAffectRoughness",
                   "coat coat_color coat_roughness coat_anisotropy coat_rotation coat_IOR coat_normal coat_affect_color coat_affect_roughness")},
        {"Sheen", s("sheen sheenColor sheenRoughness", "sheen sheen_color sheen_roughness")},
        {"Emission", s("emission emissionColor", "emission emission_color")},
        {"Thin Film", s("thinFilmThickness thinFilmIOR", "thin_film_thickness thin_film_IOR")},
        {"Geometry", s("opacity thinWalled normalCamera tangentUCamera", "opacity thin_walled normalCamera tangent")},
        {"Advanced", s("caustics internalReflections exitToBackground indirectDiffuse indirectSpecular", "caustics internal_reflections exit_to_background indirect_diffuse indirect_specular")},
        {"Node Behaviour", "caching nodeState frozen"}};
}

const QList<Section>& sections_for(const QString& type) {
    static const QList<Section> xf{
        {"Transform Attributes", "translate rotate rotateOrder scale shear rotateAxis inheritsTransform"},
        {"Pivots", "rotatePivot scalePivot rotatePivotTranslate scalePivotTranslate displayRotatePivot displayScalePivot"},
        {"Limit Information", "minTransLimit maxTransLimit minRotLimit maxRotLimit minScaleLimit maxScaleLimit"},
        {"Display", "visibility template displayHandle displayLocalAxis intermediateObject useObjectColor objectColor wireColorRGB useOutlinerColor outlinerColor hiddenInOutliner"},
        {"Drawing Overrides", kDrawingOverrides},
        {"Node Behaviour", "caching nodeState frozen"}};
    static const QList<Section> joint{
        {"Transform Attributes", "translate rotate rotateOrder scale shear inheritsTransform"},
        {"Joint", "jointOrient jointType segmentScaleCompensate preferredAngle stiffness radius drawStyle side type otherType"},
        {"Limit Information", "minRotLimit maxRotLimit minTransLimit maxTransLimit"},
        {"Display", "visibility template displayLocalAxis useObjectColor objectColor wireColorRGB useOutlinerColor outlinerColor hiddenInOutliner"},
        {"Drawing Overrides", kDrawingOverrides},
        {"Node Behaviour", "caching nodeState frozen"}};
    static const QList<Section> mesh{
        {"Mesh Component Display", "displayVertices displayEdges displayBorders displayNormal normalSize backfaceCulling"},
        {"Smooth Mesh", "displaySmoothMesh smoothLevel smoothPreview"},
        {"Symmetry", "symmetry symmetryPlane"},
        {"Render Stats", kRenderStats},
        {"Harold", kHaroldShape},
        {"Harold Subdivision", kHaroldSubdiv},
        {"Harold Displacement", kHaroldDisp},
        {"Display", "visibility template intermediateObject useObjectColor objectColor wireColorRGB useOutlinerColor outlinerColor hiddenInOutliner"},
        {"Drawing Overrides", kDrawingOverrides},
        {"Node Behaviour", "caching nodeState frozen"}};
    static const QList<Section> camera{
        {"Camera Attributes", "focalLength lensSqueezeRatio cameraScale nearClipPlane farClipPlane centerOfInterest"},
        {"Film Back", "horizontalFilmAperture verticalFilmAperture filmFit filmFitOffset horizontalFilmOffset verticalFilmOffset"},
        {"Orthographic Views", "orthographic orthographicWidth"},
        {"Harold", "renderable depthOfField fStop focusDistance focusRegionScale shutterAngle motionBlur"},
        {"Display Options", "displayFilmGate displayResolution displayGateMask overscan"},
        {"Display", "visibility"}};
    static const QList<Section> ambient{
        {"Ambient Light Attributes", "color intensity ambientShade"},
        {"Shadows", "shadowColor useRayTraceShadows shadowRays rayDepthLimit castSoftShadows shadowRadius receiveShadows"},
        {"Harold", kHaroldLight},
        {"Display", "visibility locatorScale"},
        {"Node Behaviour", "caching nodeState frozen"}};
    static const QList<Section> directional{
        {"Directional Light Attributes", "color intensity emitDiffuse emitSpecular"},
        {"Shadows", "shadowColor useDepthMapShadows useRayTraceShadows lightAngle shadowRays rayDepthLimit dmapResolution dmapFilterSize dmapBias"},
        {"Harold", kHaroldLight},
        {"Display", "visibility locatorScale"},
        {"Node Behaviour", "caching nodeState frozen"}};
    static const QList<Section> point{
        {"Point Light Attributes", "color intensity emitDiffuse emitSpecular decayRate"},
        {"Light Effects", "fogType fogRadius fogIntensity"},
        {"Shadows", "shadowColor useDepthMapShadows useRayTraceShadows lightRadius shadowRays rayDepthLimit dmapResolution dmapFilterSize dmapBias"},
        {"Harold", kHaroldLight},
        {"Display", "visibility locatorScale"},
        {"Node Behaviour", "caching nodeState frozen"}};
    static const QList<Section> spot{
        {"Spot Light Attributes", "color intensity emitDiffuse emitSpecular decayRate coneAngle penumbraAngle dropoff"},
        {"Light Effects", "fogSpread fogIntensity barnDoors leftBarnDoor rightBarnDoor topBarnDoor bottomBarnDoor useDecayRegions startDistance1 endDistance1 startDistance2 endDistance2 startDistance3 endDistance3"},
        {"Shadows", "shadowColor useDepthMapShadows useRayTraceShadows lightRadius shadowRays rayDepthLimit dmapResolution dmapFilterSize dmapBias"},
        {"Harold", kHaroldLight},
        {"Display", "visibility locatorScale"},
        {"Node Behaviour", "caching nodeState frozen"}};
    static const QList<Section> area{
        {"Area Light Attributes", "color intensity emitDiffuse emitSpecular decayRate normalize"},
        {"Shadows", "shadowColor useDepthMapShadows useRayTraceShadows shadowRays rayDepthLimit dmapResolution dmapFilterSize dmapBias"},
        {"Harold", kHaroldLight},
        {"Display", "visibility locatorScale"},
        {"Node Behaviour", "caching nodeState frozen"}};
    static const QList<Section> volume{
        {"Volume Light Attributes", "color intensity lightShape volumeLightDir arc coneEndRadius emitAmbient emitDiffuse emitSpecular"},
        {"Shadows", "shadowColor useRayTraceShadows lightRadius shadowRays rayDepthLimit"},
        {"Harold", kHaroldLight},
        {"Display", "visibility locatorScale"},
        {"Node Behaviour", "caching nodeState frozen"}};
    static const QList<Section> curve{
        {"NURBS Curve History", "degree form spans"},
        {"Component Display", "dispCV dispEP dispHull lineWidth"},
        {"Display", "visibility template intermediateObject useObjectColor objectColor wireColorRGB useOutlinerColor outlinerColor hiddenInOutliner"},
        {"Drawing Overrides", kDrawingOverrides},
        {"Node Behaviour", "caching nodeState frozen"}};
    static const QList<Section> skin{
        {"Skin Cluster Attributes", "envelope skinningMethod useComponents maxInfluences maintainMaxInfluences normalizeWeights weightDistribution deformUserNormals"},
        {"Node Behaviour", "caching nodeState frozen"}};
    static const QList<Section> material{
        {"Common Material Attributes", "color outColor transparency outTransparency ambientColor incandescence normalCamera diffuse translucence translucenceDepth translucenceFocus baseColor base metalness opacity"},
        {"Specular Shading", "cosinePower eccentricity specularRollOff angle spreadX spreadY roughness highlightSize whiteness specularColor specular specularRoughness reflectivity reflectedColor reflectionLimit fresnelRefractiveIndex"},
        {"Special Effects", "hideSource glowIntensity emission emissionColor outGlowColor"},
        {"Matte Opacity", "matteOpacityMode matteOpacity outMatteOpacity"},
        {"Raytrace Options", "refractions refractiveIndex refractionLimit lightAbsorbance surfaceThickness shadowAttenuation chromaticAberration reflectionSpecularity"}};
    static const QList<Section> file{
        {"File Attributes", "fileTextureName colorSpace ignoreColorSpaceFileRules filterType filter filterOffset useFrameExtension frameExtension frameOffset uvTilingMode"},
        {"Color Balance", "defaultColor colorGain colorOffset alphaGain alphaOffset alphaIsLuminance invert exposure"},
        {"Effects", "fileHasAlpha"},
        {"2d Texture Placement", "coverage translateFrame rotateFrame mirrorU mirrorV wrapU wrapV stagger repeatUV offset rotateUV noiseUV"}};
    static const QList<Section> place{{"2d Texture Placement Attributes", "coverage translateFrame rotateFrame mirrorU mirrorV wrapU wrapV stagger repeatUV offset rotateUV noiseUV"}};
    static const QList<Section> group{{"Shading Group Attributes", "surfaceShader volumeShader displacementShader imageShader renderableOnlySet annotation"}};
    static const QList<Section> other{{"Node Behaviour", "caching nodeState frozen"}};
    static const QList<Section> shape{
        {"Display", "visibility template intermediateObject useObjectColor objectColor wireColorRGB useOutlinerColor outlinerColor hiddenInOutliner"},
        {"Drawing Overrides", kDrawingOverrides},
        {"Node Behaviour", "caching nodeState frozen"}};
    static const QList<Section> surface{
        {"Render Stats", kRenderStats},
        {"Harold", kHaroldShape},
        {"Harold Subdivision", kHaroldSubdiv},
        {"Harold Displacement", kHaroldDisp},
        {"Display", "visibility template intermediateObject useObjectColor objectColor wireColorRGB useOutlinerColor outlinerColor hiddenInOutliner"},
        {"Drawing Overrides", kDrawingOverrides},
        {"Node Behaviour", "caching nodeState frozen"}};
    if (type == "nurbsSurface") return surface;
    static const QList<Section> standard = standard_sections(false);
    static const QList<Section> ai_standard = standard_sections(true);
    if (type == "standardSurface") return standard;
    if (type == "aiStandardSurface") return ai_standard;
    static const QList<Section> sky{
        {"Sky Dome Light Attributes", "color intensity aiExposure shadowColor resolution aiResolution format camera aiCamera skyRadius"},
        {"Harold", kHaroldLight},
        {"Display", "visibility locatorScale"},
        {"Node Behaviour", "caching nodeState frozen"}};
    static const QList<Section> ai_area{
        {"Area Light Attributes", "color intensity aiExposure shadowColor aiTranslator aiSpread aiRoundness aiSoftEdge"},
        {"Harold", kHaroldLight},
        {"Display", "visibility locatorScale"},
        {"Node Behaviour", "caching nodeState frozen"}};
    static const QList<Section> mesh_light{
        {"Mesh Light Attributes", "color intensity aiExposure shadowColor lightVisible"},
        {"Harold", kHaroldLight},
        {"Display", "visibility locatorScale"},
        {"Node Behaviour", "caching nodeState frozen"}};
    static const QList<Section> photometric{
        {"Photometric Light Attributes", "color intensity aiExposure shadowColor aiFilename"},
        {"Harold", kHaroldLight},
        {"Display", "visibility locatorScale"},
        {"Node Behaviour", "caching nodeState frozen"}};
    if (type == "aiSkyDomeLight") return sky;
    if (type == "aiAreaLight") return ai_area;
    if (type == "aiMeshLight") return mesh_light;
    if (type == "aiPhotometricLight") return photometric;
    static const QSet<QString> shapes{"nurbsSurface", "lattice", "baseLattice", "deformHandle", "softModHandle", "follicle", "distanceDimShape", "annotationShape", "motionTrailShape", "ikEffector"};
    if (shapes.contains(type)) return shape;
    static const QSet<QString> materials{"lambert", "blinn", "phong", "phongE", "anisotropic", "surfaceShader"};
    if (materials.contains(type)) return material;
    if (type == "file") return file;
    if (type == "place2dTexture") return place;
    if (type == "shadingEngine") return group;
    if (type == "transform" || type == "ikHandle" || type == "locator") return xf;
    if (type == "joint") return joint;
    if (type == "mesh") return mesh;
    if (type == "camera") return camera;
    if (type == "ambientLight") return ambient;
    if (type == "directionalLight") return directional;
    if (type == "pointLight") return point;
    if (type == "spotLight") return spot;
    if (type == "areaLight") return area;
    if (type == "volumeLight") return volume;
    if (type == "nurbsCurve") return curve;
    if (type == "skinCluster") return skin;
    return other;
}

// A slider as the reference's: a press anywhere on the track puts the handle there and the drag goes on from it.
class AttrSlider : public QSlider {
public:
    using QSlider::QSlider;

protected:
    void mousePressEvent(QMouseEvent* e) override {
        if (e->button() == Qt::LeftButton) {
            jump(e);
            setSliderDown(true);
            e->accept();
            return;
        }
        QSlider::mousePressEvent(e);
    }
    void mouseMoveEvent(QMouseEvent* e) override {
        if (isSliderDown() && (e->buttons() & Qt::LeftButton)) {
            jump(e);
            e->accept();
            return;
        }
        QSlider::mouseMoveEvent(e);
    }
    void mouseReleaseEvent(QMouseEvent* e) override {
        if (e->button() == Qt::LeftButton && isSliderDown()) {
            setSliderDown(false);
            e->accept();
            return;
        }
        QSlider::mouseReleaseEvent(e);
    }
    void wheelEvent(QWheelEvent* e) override { e->ignore(); }

private:
    void jump(QMouseEvent* e) {
        const int handle = 10;
        int v = QStyle::sliderValueFromPosition(minimum(), maximum(), int(e->position().x()) - handle / 2, qMax(1, width() - handle));
        if (v != sliderPosition()) {
            setSliderPosition(v);
            emit sliderMoved(v);
        }
    }
};

// The map button's two faces: a checker where a texture can go, an arrow into a box where something is connected.
QIcon map_icon(bool connected) {
    QPixmap px(14, 14);
    px.fill(Qt::transparent);
    QPainter p(&px);
    if (!connected) {
        p.fillRect(1, 1, 12, 12, QColor(20, 20, 20));
        p.fillRect(1, 1, 6, 6, QColor(235, 235, 235));
        p.fillRect(7, 7, 6, 6, QColor(235, 235, 235));
    } else {
        p.setPen(QPen(QColor(225, 225, 225), 1.2));
        p.drawRect(5, 2, 8, 10);
        p.drawLine(0, 7, 8, 7);
        p.drawLine(8, 7, 5, 4);
        p.drawLine(8, 7, 5, 10);
    }
    return QIcon(px);
}

void break_connection(const QString& plug) {
    QJsonArray from = ask("listConnections", QJsonObject{{"args", QJsonArray{plug}}, {"plugs", true}, {"destination", false}}).toArray();
    if (!from.isEmpty()) exec("disconnectAttr", args(QJsonArray{from.at(0), plug}));
}

// The button at the end of a row that a texture can drive: it opens Create Render Node, or goes to what is connected.
QToolButton* map_button(const QJsonObject& a, QWidget* parent) {
    QString plug = a["plug"].toString(), source = a["source"].toString();
    auto* b = new QToolButton(parent);
    b->setObjectName("mapButton");
    b->setAutoRaise(true);
    b->setFixedSize(18, 18);
    b->setIcon(map_icon(!source.isEmpty()));
    b->setToolTip(source.isEmpty() ? "Create a render node and connect it to this attribute" : "Go to input connection: " + source);
    b->setContextMenuPolicy(Qt::CustomContextMenu);
    QObject::connect(b, &QToolButton::clicked, b, [b, plug, source] {
        if (!source.isEmpty()) {
            exec("select", args(QJsonArray{source}));
            return;
        }
        create_render_node(b->window(), plug);
    });
    QObject::connect(b, &QToolButton::customContextMenuRequested, b, [b, plug, source](const QPoint& at) {
        if (source.isEmpty()) return;
        QMenu m;
        m.addAction("Break Connection", [plug] { break_connection(plug); });
        m.exec(b->mapToGlobal(at));
    });
    return b;
}

// Slider ranges widened by a value typed past them, kept while the program runs, as the reference's sliders keep theirs.
QHash<QString, QPair<double, double>>& widened() {
    static QHash<QString, QPair<double, double>> w;
    return w;
}

// A number with a slider beside it, when the attribute has a range. The slider runs over the soft range; a value typed
// past it, within the attribute's limits, widens the slider so the value sits half way.
QWidget* ranged_field(const QJsonObject& a, double lo, double hi, QWidget* parent) {
    QString plug = a["plug"].toString(), type = a["type"].toString();
    const double value = a["value"].toDouble();
    if (widened().contains(plug)) {
        lo = qMin(lo, widened()[plug].first);
        hi = qMax(hi, widened()[plug].second);
    }
    if (value > hi) hi = value * 2.0;
    if (value < lo) lo = value < 0 ? value * 2.0 : value;
    if (a["hardMax"].isDouble()) hi = qMin(hi, a["hardMax"].toDouble());
    if (a["hardMin"].isDouble()) lo = qMax(lo, a["hardMin"].toDouble());
    widened()[plug] = {lo, hi};
    auto* row = new QWidget(parent);
    auto* h = new QHBoxLayout(row);
    h->setContentsMargins(0, 0, 0, 0);
    h->setSpacing(4);
    auto* e = new QLineEdit(type == "int" ? QString::number(a["value"].toInteger()) : fmt3(value), row);
    e->setFixedWidth(54);
    auto* s = new AttrSlider(Qt::Horizontal, row);
    s->setMinimumWidth(8);
    s->setRange(0, 1000);
    auto to_pos = [lo, hi](double v) { return int(qBound(0.0, (v - lo) / qMax(1e-9, hi - lo), 1.0) * 1000 + 0.5); };
    s->setValue(to_pos(value));
    bool ro = a["output"].toBool() || a["connected"].toBool();
    e->setReadOnly(ro);
    if (a["connected"].toBool()) e->setObjectName("connectedField");
    s->setEnabled(!ro);
    bool is_int = type == "int";
    auto value_of = [lo, hi, is_int](int p) {
        double v = lo + (hi - lo) * p / 1000.0;
        return is_int ? QJsonValue(qint64(std::round(v))) : QJsonValue(v);
    };
    QObject::connect(s, &QSlider::sliderMoved, e, [e, value_of, plug, is_int](int p) {
        QJsonValue v = value_of(p);
        e->setText(is_int ? QString::number(v.toInteger()) : fmt3(v.toDouble()));
        exec_quiet("setAttr", args(QJsonArray{plug, v}));
        Host::get()->poke();
    });
    QObject::connect(s, &QSlider::sliderReleased, e, [s, value_of, plug] { exec("setAttr", args(QJsonArray{plug, value_of(s->value())})); });
    QObject::connect(e, &QLineEdit::textEdited, e, [e] { e->setProperty("editing", true); });
    QObject::connect(e, &QLineEdit::editingFinished, e, [e, plug, is_int] {
        if (!e->property("editing").toBool()) return;
        e->setProperty("editing", false);
        exec("setAttr", args(QJsonArray{plug, is_int ? QJsonValue(qint64(e->text().toDouble())) : QJsonValue(e->text().toDouble())}));
    });
    h->addWidget(e);
    h->addWidget(s, 1);
    if (a["mappable"].toBool()) h->addWidget(map_button(a, row));
    return row;
}

// A linear colour as the swatch shows it.
QColor swatch_color(const QJsonArray& v) {
    auto enc = [](double x) { return qBound(0, int(std::pow(qBound(0.0, x, 1.0), 1.0 / 2.2) * 255.0 + 0.5), 255); };
    return QColor(enc(v.at(0).toDouble()), enc(v.at(1).toDouble()), enc(v.at(2).toDouble()));
}

// The usual network behind a slot: a file node, its placement, the image, and its connection.
void make_file_texture(const QString& plug, const QString& path) {
    QString file = exec("shadingNode", QJsonObject{{"args", QJsonArray{"file"}}, {"asTexture", true}}).toString();
    QString place = exec("shadingNode", QJsonObject{{"args", QJsonArray{"place2dTexture"}}, {"asUtility", true}}).toString();
    if (file.isEmpty() || place.isEmpty()) return;
    exec("defaultNavigation", QJsonObject{{"connectToExisting", true}, {"source", place}, {"destination", file}});
    set_plug(file + ".fileTextureName", path, "string");
    exec("defaultNavigation", QJsonObject{{"connectToExisting", true}, {"source", file}, {"destination", plug}});
}

const char* kImageFilter = "Images (*.png *.jpg *.jpeg *.tga *.bmp *.tif *.tiff);;All Files (*)";

// A colour: its swatch, and the slot a texture goes in.
QWidget* color_field(const QJsonObject& a, QWidget* parent) {
    QString plug = a["plug"].toString();
    bool ro = a["output"].toBool() || a["connected"].toBool();
    auto* w = new QWidget(parent);
    auto* h = new QHBoxLayout(w);
    h->setContentsMargins(0, 0, 0, 0);
    h->setSpacing(3);
    auto* swatch = new QPushButton(w);
    swatch->setFixedSize(54, 18);
    bool stored = a["displayColor"].toBool();
    QJsonArray value = a["value"].toArray();
    QColor shown = stored ? QColor::fromRgbF(value.at(0).toDouble(), value.at(1).toDouble(), value.at(2).toDouble()) : swatch_color(value);
    swatch->setStyleSheet(QString("background-color: %1; border: 1px solid #222;").arg(shown.name()));
    swatch->setEnabled(!ro);
    swatch->setObjectName("colorSwatch");
    QObject::connect(swatch, &QPushButton::clicked, swatch, [swatch, plug, shown, stored] {
        QColor c = QColorDialog::getColor(shown, swatch, "Color");
        if (!c.isValid()) return;
        auto dec = [stored](int x) { return stored ? x / 255.0 : std::pow(x / 255.0, 2.2); };
        exec("setAttr", QJsonObject{{"args", QJsonArray{plug, dec(c.red()), dec(c.green()), dec(c.blue())}}, {"type", "double3"}});
    });
    h->addWidget(swatch);
    // The slider runs the colour's value from black to full, keeping its hue and saturation.
    auto* s = new AttrSlider(Qt::Horizontal, w);
    s->setMinimumWidth(8);
    s->setRange(0, 1000);
    s->setValue(int(shown.valueF() * 1000 + 0.5));
    s->setEnabled(!ro);
    auto colour_at = [shown, stored](int p) {
        QColor c = QColor::fromHsvF(qMax(0.0f, shown.hsvHueF()), shown.hsvSaturationF(), p / 1000.0);
        auto dec = [stored](double x) { return stored ? x : std::pow(x, 2.2); };
        return QJsonArray{dec(c.redF()), dec(c.greenF()), dec(c.blueF())};
    };
    QObject::connect(s, &QSlider::sliderMoved, swatch, [swatch, colour_at, plug, shown](int p) {
        QJsonArray c = colour_at(p);
        swatch->setStyleSheet(QString("background-color: %1; border: 1px solid #222;").arg(QColor::fromHsvF(qMax(0.0f, shown.hsvHueF()), shown.hsvSaturationF(), p / 1000.0).name()));
        exec_quiet("setAttr", QJsonObject{{"args", QJsonArray{plug, c.at(0), c.at(1), c.at(2)}}, {"type", "double3"}});
        Host::get()->poke();
    });
    QObject::connect(s, &QSlider::sliderReleased, swatch, [s, colour_at, plug] {
        QJsonArray c = colour_at(s->value());
        exec("setAttr", QJsonObject{{"args", QJsonArray{plug, c.at(0), c.at(1), c.at(2)}}, {"type", "double3"}});
    });
    h->addWidget(s, 1);
    if (a["mappable"].toBool()) h->addWidget(map_button(a, w));
    return w;
}

// A file node's path: what is written, a browse button, and where it was found.
QWidget* file_field(const QJsonObject& a, QWidget* parent) {
    QString plug = a["plug"].toString();
    auto* w = new QWidget(parent);
    auto* v = new QVBoxLayout(w);
    v->setContentsMargins(0, 0, 0, 0);
    v->setSpacing(2);
    auto* row = new QHBoxLayout();
    row->setSpacing(3);
    auto* e = new QLineEdit(a["value"].toString(), w);
    QObject::connect(e, &QLineEdit::textEdited, e, [e] { e->setProperty("editing", true); });
    QObject::connect(e, &QLineEdit::editingFinished, e, [e, plug] {
        if (!e->property("editing").toBool()) return;
        e->setProperty("editing", false);
        set_plug(plug, e->text(), "string");
    });
    auto* browse = new QToolButton(w);
    browse->setIcon(icon("open", 16));
    browse->setToolTip("Browse for an image");
    QObject::connect(browse, &QToolButton::clicked, browse, [browse, plug] {
        QString path = QFileDialog::getOpenFileName(browse, "Choose an image", QString(), kImageFilter);
        if (!path.isEmpty()) set_plug(plug, path, "string");
    });
    row->addWidget(e, 1);
    row->addWidget(browse);
    v->addLayout(row);
    QJsonObject where = query("texturePath", plug.section('.', 0, 0)).toObject();
    QString found = where["resolved"].toString();
    QString written = where["written"].toString();
    if (!written.isEmpty()) {
        auto* note = new QLabel(found.isEmpty() ? QString("Not found") : found == written ? QString() : "Found: " + found, w);
        note->setWordWrap(true);
        if (!note->text().isEmpty()) v->addWidget(note);
    }
    return w;
}

// The override colour's index: its swatch, which opens the palette, and a slider through the 32 entries.
QWidget* color_index_field(const QJsonObject& a, QWidget* parent) {
    QString plug = a["plug"].toString();
    bool ro = a["output"].toBool() || a["connected"].toBool();
    int index = a["value"].toInt();
    auto* w = new QWidget(parent);
    auto* h = new QHBoxLayout(w);
    h->setContentsMargins(0, 0, 0, 0);
    h->setSpacing(4);
    auto* swatch = new QPushButton(w);
    swatch->setObjectName("colorSwatch");
    swatch->setFixedSize(36, 18);
    swatch->setStyleSheet(QString("background-color: %1; border: 1px solid #222;").arg(palette_color(index).name()));
    swatch->setEnabled(!ro);
    QObject::connect(swatch, &QPushButton::clicked, swatch, [swatch, plug, index] {
        ColorChoice c = choose_display_color(swatch, "Color", index, false, Qt::red);
        if (!c.accepted) return;
        if (c.use_rgb) {
            QString node = plug.left(plug.lastIndexOf('.'));
            set_plug(node + ".overrideRGBColors", true, "bool");
            exec("setAttr", QJsonObject{{"args", QJsonArray{node + ".overrideColorRGB", c.rgb.redF(), c.rgb.greenF(), c.rgb.blueF()}}, {"type", "double3"}});
        } else {
            set_plug(plug, c.index, "int");
        }
    });
    auto* s = new QSlider(Qt::Horizontal, w);
    s->setRange(0, 31);
    s->setValue(index);
    s->setEnabled(!ro);
    auto* number = new QLabel(QString::number(index), w);
    number->setFixedWidth(20);
    QObject::connect(s, &QSlider::sliderMoved, number, [number, swatch](int v) {
        number->setText(QString::number(v));
        swatch->setStyleSheet(QString("background-color: %1; border: 1px solid #222;").arg(palette_color(v).name()));
    });
    QObject::connect(s, &QSlider::sliderReleased, s, [s, plug] { set_plug(plug, s->value(), "int"); });
    h->addWidget(swatch);
    h->addWidget(number);
    h->addWidget(s, 1);
    return w;
}

QWidget* attr_field(const QJsonObject& a, QWidget* parent) {
    QString type = a["type"].toString(), plug = a["plug"].toString();
    bool ro = a["output"].toBool() || a["connected"].toBool();
    if (a["attr"].toString() == "overrideColor") return color_index_field(a, parent);
    if (type == "vec3" && a["color"].toBool()) return color_field(a, parent);
    if (type == "string" && a["file"].toBool() && !ro) return file_field(a, parent);
    if (type == "bool") {
        auto* cb = new QCheckBox(parent);
        cb->setChecked(a["value"].toBool());
        cb->setEnabled(!ro);
        QObject::connect(cb, &QCheckBox::toggled, cb, [plug](bool on) { set_plug(plug, on, "bool"); });
        return cb;
    }
    if (type == "enum") {
        auto* box = new QComboBox(parent);
        for (const QJsonValue& n : a["enum"].toArray()) box->addItem(n.toString());
        box->setCurrentIndex(a["value"].toInt());
        box->setEnabled(!ro);
        QObject::connect(box, &QComboBox::activated, box, [plug](int i) { set_plug(plug, i, "int"); });
        return box;
    }
    if (type == "vec3") {
        auto* w = new QWidget(parent);
        auto* h = new QHBoxLayout(w);
        h->setContentsMargins(0, 0, 0, 0);
        h->setSpacing(3);
        QJsonArray v = a["value"].toArray();
        QJsonArray kids = a["children"].toArray();
        for (int k = 0; k < (a["pair"].toBool() ? 2 : 3); ++k) {
            auto* e = new QLineEdit(fmt3(v.at(k).toDouble()), w);
            // Three to a row: thin padding, so a value shows whole at the panel's usual width.
            e->setStyleSheet("padding-left: 1px; padding-right: 0px;");
            e->setMinimumWidth(38);
            e->setReadOnly(ro);
            QString child = kids.at(k).toString();
            if (child.isEmpty()) child = a["attr"].toString() + QString("XYZ")[k];
            QString cp = plug.left(plug.lastIndexOf('.')) + "." + child;
            QObject::connect(e, &QLineEdit::textEdited, e, [e] { e->setProperty("editing", true); });
            QObject::connect(e, &QLineEdit::editingFinished, e, [e, cp] {
                if (!e->property("editing").toBool()) return;
                e->setProperty("editing", false);
                set_plug(cp, e->text().toDouble(), "float");
            });
            h->addWidget(e);
        }
        return w;
    }
    if (type == "float" || type == "int") {
        if (a["min"].isDouble() && a["max"].isDouble() && a["max"].toDouble() > a["min"].toDouble()) return ranged_field(a, a["min"].toDouble(), a["max"].toDouble(), parent);
        auto* e = new QLineEdit(type == "int" ? QString::number(a["value"].toInteger()) : fmt3(a["value"].toDouble()), parent);
        e->setReadOnly(ro);
        bool is_int = type == "int";
        QObject::connect(e, &QLineEdit::textEdited, e, [e] { e->setProperty("editing", true); });
        QObject::connect(e, &QLineEdit::editingFinished, e, [e, plug, is_int] {
            if (!e->property("editing").toBool()) return;
            e->setProperty("editing", false);
            set_plug(plug, is_int ? QJsonValue(qint64(e->text().toDouble())) : QJsonValue(e->text().toDouble()), "float");
        });
        if (!a["mappable"].toBool()) return e;
        // A number a texture can drive: the field, then the map button at the row's end where the sliders' are.
        if (a["connected"].toBool()) e->setObjectName("connectedField");
        auto* w = new QWidget(parent);
        auto* h = new QHBoxLayout(w);
        h->setContentsMargins(0, 0, 0, 0);
        h->setSpacing(4);
        e->setParent(w);
        e->setFixedWidth(54);
        h->addWidget(e);
        h->addStretch(1);
        h->addWidget(map_button(a, w));
        return w;
    }
    if (type == "string") {
        auto* e = new QLineEdit(a["value"].toString(), parent);
        e->setReadOnly(ro);
        QObject::connect(e, &QLineEdit::textEdited, e, [e] { e->setProperty("editing", true); });
        QObject::connect(e, &QLineEdit::editingFinished, e, [e, plug] {
            if (!e->property("editing").toBool()) return;
            e->setProperty("editing", false);
            set_plug(plug, e->text(), "string");
        });
        return e;
    }
    return new QLabel(type == "matrix" ? "(matrix)" : "", parent);
}

// A row's name in a column of one width down the whole tab, so the fields line up from frame to frame. A name too long
// for it loses its start, as the reference's does; the whole of it is in the tooltip.
class RowLabel : public QLabel {
public:
    RowLabel(const QString& text, QWidget* parent) : QLabel(text, parent) {
        setFixedWidth(112);
        setToolTip(text);
    }

protected:
    void paintEvent(QPaintEvent*) override {
        QPainter p(this);
        p.setPen(palette().color(isEnabled() ? QPalette::Active : QPalette::Disabled, QPalette::WindowText));
        p.drawText(rect(), Qt::AlignRight | Qt::AlignVCenter, fontMetrics().elidedText(text(), Qt::ElideLeft, width()));
    }
};

// A row's name. A right click on it gives what the reference's gives there, for what this attribute can take.
QLabel* attr_label(const QString& text, const QJsonObject& a, QWidget* parent, QWidget* field_widget) {
    QPointer<QWidget> field = field_widget;
    auto* l = new RowLabel(text, parent);
    l->setContextMenuPolicy(Qt::CustomContextMenu);
    const QString plug = a["plug"].toString(), source = a["source"].toString();
    const bool mappable = a["mappable"].toBool(), connected = a["connected"].toBool(), output = a["output"].toBool();
    const bool colour = a["type"].toString() == "vec3" && a["color"].toBool();
    QObject::connect(l, &QLabel::customContextMenuRequested, l, [=](const QPoint& at) {
        QMenu m;
        if (!source.isEmpty()) {
            m.addAction(source + "...", [source] { exec("select", args(QJsonArray{source})); });
            m.addSeparator();
        }
        if (!output) m.addAction("Set Key", [plug] { exec("setKeyframe", args(QJsonArray{plug})); })->setEnabled(!connected || source.startsWith("animCurve"));
        if (connected) m.addAction("Break Connection", [plug] { break_connection(plug); });
        if (mappable && !connected) m.addAction("Create New Texture...", [l, plug] { create_render_node(l->window(), plug); });
        if (colour && !connected && !output) {
            m.addAction("Color Chooser...", [field] {
                if (!field) return;
                if (auto* b = field->findChild<QPushButton*>("colorSwatch")) b->click();
            });
        }
        if (!output) {
            const bool locked = ask("getAttr", QJsonObject{{"args", QJsonArray{plug}}, {"lock", true}}).toBool();
            m.addAction(locked ? "Unlock Attribute" : "Lock Attribute", [plug, locked] { exec("setAttr", QJsonObject{{"args", QJsonArray{plug}}, {"lock", !locked}}); });
        }
        if (!m.isEmpty()) m.exec(l->mapToGlobal(at));
    });
    return l;
}

// A standard surface's rows as the reference names them: inside the Specular frame, specularRoughness is Roughness and
// the frame's own weight is Weight.
QString material_label(const QString& section, const QString& attr, const QString& nice) {
    static const QHash<QString, QString> named{{"transmissionDispersion", "Dispersion Abbe"}, {"transmission_dispersion", "Dispersion Abbe"}, {"normalCamera", "Bump Mapping"},
                                               {"tangentUCamera", "Anisotropy Tangent"}, {"tangent", "Anisotropy Tangent"}, {"coatNormal", "Normal"}, {"coat_normal", "Normal"}};
    if (named.contains(attr)) return named.value(attr);
    QString camel = section;
    camel.remove(' ');
    if (camel.isEmpty()) return nice;
    camel[0] = camel[0].toLower();
    QString snake = section.toLower().replace(' ', '_');
    QString rest;
    if (attr == camel || attr == snake) return "Weight";
    if (attr.startsWith(camel) && attr.size() > camel.size() && attr[camel.size()].isUpper()) rest = attr.mid(camel.size());
    else if (attr.startsWith(snake + "_")) rest = attr.mid(snake.size() + 1);
    else return nice;
    if (rest == "IOR") return rest;
    QString out;
    for (int i = 0; i < rest.size(); ++i) {
        if (rest[i] == '_') {
            out += ' ';
        } else if (i == 0 || rest[i - 1] == '_') {
            out += rest[i].toUpper();
        } else {
            if (rest[i].isUpper() && !rest[i - 1].isUpper()) out += ' ';
            out += rest[i];
        }
    }
    return out;
}

// A collapsible section header, remembered per title across refreshes.
QSet<QString>& closed_sections() {
    static QSet<QString> s{"Node Behaviour", "Limit Information", "Extra Attributes"};
    return s;
}

QFormLayout* add_section(QVBoxLayout* v, QWidget* parent, const QString& title) {
    auto* head = new QToolButton(parent);
    head->setObjectName("aeSectionButton");
    head->setText(title);
    head->setCheckable(true);
    bool open = !closed_sections().contains(title);
    head->setChecked(open);
    head->setArrowType(open ? Qt::DownArrow : Qt::RightArrow);
    head->setToolButtonStyle(Qt::ToolButtonTextBesideIcon);
    head->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    v->addWidget(head);
    auto* box = new QWidget(parent);
    auto* f = new QFormLayout(box);
    f->setLabelAlignment(Qt::AlignRight);
    f->setContentsMargins(4, 2, 2, 6);
    f->setHorizontalSpacing(4);
    f->setVerticalSpacing(3);
    box->setVisible(open);
    v->addWidget(box);
    QObject::connect(head, &QToolButton::toggled, box, [box, head, title](bool on) {
        box->setVisible(on);
        head->setArrowType(on ? Qt::DownArrow : Qt::RightArrow);
        if (on) closed_sections().remove(title);
        else closed_sections().insert(title);
    });
    return f;
}

} // namespace

AttributeEditor::AttributeEditor(QWidget* parent, const QString& pinned) : Panel(parent), pinned_(pinned) {
    auto* lay = new QVBoxLayout(this);
    lay->setContentsMargins(2, 2, 2, 2);
    lay->setSpacing(2);
    // List, Selected, Focus, Attributes, as a row of buttons above the tabs.
    auto* top = new QHBoxLayout();
    auto* back = new QToolButton(this);
    back->setArrowType(Qt::LeftArrow);
    back->setToolTip("The node before this one in the chain");
    auto* fwd = new QToolButton(this);
    fwd->setArrowType(Qt::RightArrow);
    fwd->setToolTip("The node after this one in the chain");
    connect(back, &QToolButton::clicked, this, [this] { tabs_->setCurrentIndex(qMax(0, tabs_->currentIndex() - 1)); });
    connect(fwd, &QToolButton::clicked, this, [this] { tabs_->setCurrentIndex(qMin(tabs_->count() - 1, tabs_->currentIndex() + 1)); });
    top->addWidget(back);
    top->addWidget(fwd);
    top->addStretch(1);
    lay->addLayout(top);
    tabs_ = new QTabWidget(this);
    tabs_->setDocumentMode(true);
    tabs_->setUsesScrollButtons(true);
    lay->addWidget(tabs_, 1);
    connect(tabs_, &QTabWidget::currentChanged, this, [this](int i) {
        if (i >= 0) current_tab_ = tabs_->tabText(i);
    });
    auto* bottom = new QHBoxLayout();
    auto* sel = new QPushButton("Select", this);
    auto* load = new QPushButton("Load Attributes", this);
    auto* copy = new QPushButton("Copy Tab", this);
    bottom->addWidget(sel);
    bottom->addWidget(load);
    bottom->addWidget(copy);
    lay->addLayout(bottom);
    connect(sel, &QPushButton::clicked, this, [this] {
        QWidget* w = tabs_->currentWidget();
        if (w) exec("select", args(QJsonArray{w->property("nodePath").toString()}));
    });
    connect(load, &QPushButton::clicked, this, [this] { refresh(); });
    connect(copy, &QPushButton::clicked, this, [this] {
        QWidget* w = tabs_->currentWidget();
        if (!w) return;
        // A window of its own that stays on this node.
        auto* win = new AttributeEditor(main_window_widget(), w->property("nodePath").toString());
        win->setAttribute(Qt::WA_DeleteOnClose);
        win->setWindowFlags(editor_window_flags());
        win->setWindowTitle(tabs_->tabText(tabs_->currentIndex()));
        win->resize(380, 560);
        win->show();
    });
}

void AttributeEditor::refresh() {
    QJsonObject ae = query("attributeEditor", pinned_).toObject();
    const QByteArray now = QJsonDocument(ae).toJson(QJsonDocument::Compact);
    if (now == last_ && tabs_->count()) return;
    // A slider being dragged changes the value this is built from: rebuilding would take the slider from under the
    // mouse. The release comes back here.
    for (QSlider* sl : findChildren<QSlider*>())
        if (sl->isSliderDown()) return;
    last_ = now;
    QString keep = current_tab_;
    tabs_->blockSignals(true);
    while (tabs_->count()) {
        QWidget* w = tabs_->widget(0);
        tabs_->removeTab(0);
        w->deleteLater();
    }
    int select = 0;
    for (const QJsonValue& tv : ae["tabs"].toArray()) {
        QJsonObject t = tv.toObject();
        QString node_type = t["type"].toString();
        auto* scroll = new QScrollArea();
        scroll->setWidgetResizable(true);
        scroll->setFrameShape(QFrame::NoFrame);
        // Rows fit the panel's width: the sliders give, so the map buttons at the rows' ends stay in sight.
        scroll->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
        auto* body = new QWidget();
        // Narrower than its rows would like, it still takes the panel's width: the sliders are what give way.
        body->setMinimumWidth(40);
        auto* v = new QVBoxLayout(body);
        v->setContentsMargins(3, 6, 3, 6);
        v->setSpacing(2);
        auto* head = new QHBoxLayout();
        // Harold's node types carry the ai prefix of the files they are read from: the heading says whose they are.
        const bool harold_type = node_type.startsWith("ai") && node_type.size() > 2 && node_type[2].isUpper();
        head->addWidget(new QLabel(QString("<b>%1:</b>").arg(harold_type ? "harold" + node_type.mid(2) : node_type), body));
        auto* name = new QLineEdit(t["name"].toString(), body);
        name->setMinimumWidth(40);
        QString path = t["path"].toString();
        scroll->setProperty("nodePath", path);
        connect(name, &QLineEdit::editingFinished, name, [name, path] {
            if (name->text().trimmed() != path.section('|', -1)) exec("rename", args(QJsonArray{path, name->text().trimmed()}));
        });
        head->addWidget(name, 1);
        auto* focus = new QToolButton(body);
        focus->setIcon(icon("frameSel", 16));
        focus->setToolTip("Focus: select this node");
        connect(focus, &QToolButton::clicked, focus, [path] { exec("select", args(QJsonArray{path})); });
        head->addWidget(focus);
        v->addLayout(head);
        // A light's type, which swaps the shape, and whether it is in the default light set.
        static const QStringList light_types{"ambientLight", "directionalLight", "pointLight", "spotLight", "areaLight", "volumeLight"};
        if (light_types.contains(node_type)) {
            auto* form = add_section(v, body, "Light");
            auto* kind = new QComboBox(body);
            for (const QString& lt : {"Ambient", "Directional", "Point", "Spot", "Area", "Volume"}) kind->addItem(lt);
            kind->setCurrentIndex(light_types.indexOf(node_type));
            connect(kind, QOverload<int>::of(&QComboBox::activated), kind, [path](int i) {
                exec("aztekChangeLightType", args(QJsonArray{path, light_types.at(i)}));
            });
            form->addRow("Type", kind);
            auto* by_default = new QCheckBox(body);
            QString shape = path;
            by_default->setChecked(ask("aztekIlluminatesByDefault", args(QJsonArray{shape})).toBool());
            connect(by_default, &QCheckBox::toggled, by_default, [shape](bool on) {
                exec("aztekIlluminatesByDefault", QJsonObject{{"args", QJsonArray{shape}}, {"set", on}});
            });
            form->addRow("Illuminates by Default", by_default);
        }

        // Cloth: material presets.
        if (node_type == "cloth") {
            auto* form = add_section(v, body, "Presets");
            auto* preset = new QComboBox(body);
            preset->addItem("Choose a material...");
            for (const char* p : {"Silk", "Cotton", "Leather", "Heavy Denim", "Rubber"}) preset->addItem(p);
            connect(preset, QOverload<int>::of(&QComboBox::activated), preset, [path, preset](int i) {
                static const QStringList names{"", "silk", "cotton", "leather", "heavyDenim", "rubber"};
                if (i > 0) exec("clothPreset", QJsonObject{{"args", QJsonArray{path}}, {"preset", names.at(i)}});
                preset->setCurrentIndex(0);
            });
            form->addRow("Material", preset);
        }

        QJsonArray attrs = t["attrs"].toArray();
        QSet<QString> used;
        // A script's AE<type>Template lays the tab out: what it does not name shows only under -addExtraControls.
        QJsonArray layout = query("aeTemplate", node_type + " " + path).toArray();
        bool extra = layout.isEmpty();
        QFormLayout* current = nullptr;
        for (const QJsonValue& iv : layout) {
            QJsonObject item = iv.toObject();
            QString kind = item["kind"].toString();
            if (kind == "beginLayout") {
                current = add_section(v, body, item["title"].toString());
            } else if (kind == "endLayout") {
                current = nullptr;
            } else if (kind == "separator") {
                auto* line = new QFrame(body);
                line->setFrameShape(QFrame::HLine);
                if (current) current->addRow(line);
                else v->addWidget(line);
            } else if (kind == "suppress") {
                used.insert(item["attr"].toString());
            } else if (kind == "extraControls") {
                extra = true;
            } else if (kind == "control") {
                const QString n = item["attr"].toString();
                for (const QJsonValue& av : attrs) {
                    QJsonObject a = av.toObject();
                    if (a["attr"].toString() != n) continue;
                    if (!current) current = add_section(v, body, node_type);
                    used.insert(n);
                    QWidget* field = attr_field(a, body);
                    field->setToolTip(item.contains("annotation") ? item["annotation"].toString() : a["doc"].toString());
                    current->addRow(attr_label(item.contains("label") ? item["label"].toString() : a["nice"].toString(), a, body, field), field);
                }
            }
        }
        // Drawing overrides do nothing until they are enabled, and show the index or RGB colour that is in use.
        bool overrides_on = false, overrides_rgb = false;
        for (const QJsonValue& av : attrs) {
            QJsonObject a = av.toObject();
            if (a["attr"].toString() == "overrideEnabled") overrides_on = a["value"].toBool();
            if (a["attr"].toString() == "overrideRGBColors") overrides_rgb = a["value"].toBool();
        }
        for (const Section& sec : layout.isEmpty() ? sections_for(node_type) : QList<Section>()) {
            QStringList names = QString(sec.attrs).split(' ');
            QFormLayout* f = nullptr;
            bool drawing = QString(sec.title) == "Drawing Overrides";
            for (const QString& n : names)
                for (const QJsonValue& av : attrs) {
                    QJsonObject a = av.toObject();
                    if (a["attr"].toString() != n || a["type"].toString() == "matrix") continue;
                    if (!f) f = add_section(v, body, sec.title);
                    used.insert(n);
                    if (drawing && n == (overrides_rgb ? "overrideColor" : "overrideColorRGB")) continue;
                    QWidget* field = attr_field(a, body);
                    field->setToolTip(a["doc"].toString());
                    if (drawing && n != "overrideEnabled") field->setEnabled(overrides_on);
                    const bool standard = node_type == "standardSurface" || node_type == "aiStandardSurface";
                    f->addRow(attr_label(standard ? material_label(sec.title, n, a["nice"].toString()) : override_label(n, a["nice"].toString()), a, body, field), field);
                }
        }
        // The rest grouped by their own category, then Extra Attributes.
        QMap<QString, QFormLayout*> forms;
        QJsonValue notes;
        QString notes_plug = path + ".notes";
        for (const QJsonValue& av : attrs) {
            QJsonObject a = av.toObject();
            QString n = a["attr"].toString();
            if (n == "notes") {
                notes = a["value"];
                continue;
            }
            if (used.contains(n) || a["type"].toString() == "matrix" || !extra) continue;
            QString cat = a["category"].toString();
            if (cat.isEmpty()) cat = sections_for(node_type).size() > 1 ? QString("Extra Attributes") : QString("%1 Attributes").arg(node_type.left(1).toUpper() + node_type.mid(1));
            if (!forms.contains(cat)) forms[cat] = add_section(v, body, cat);
            QWidget* field = attr_field(a, body);
            field->setToolTip(a["doc"].toString());
            forms[cat]->addRow(attr_label(a["nice"].toString(), a, body, field), field);
        }
        // Notes: a string attribute added the first time it's written.
        auto* note_head = new QLabel("Notes: " + t["name"].toString(), body);
        note_head->setObjectName("aeSection");
        v->addWidget(note_head);
        auto* note = new QPlainTextEdit(notes.toString(), body);
        note->setFixedHeight(70);
        note->setProperty("editing", false);
        bool had = !notes.isUndefined();
        connect(note, &QPlainTextEdit::textChanged, note, [note] { note->setProperty("editing", true); });
        note->installEventFilter(this);
        note->setProperty("notesPlug", notes_plug);
        note->setProperty("notesNode", path);
        note->setProperty("notesHad", had);
        v->addWidget(note);
        v->addStretch(1);
        scroll->setWidget(body);
        int idx = tabs_->addTab(scroll, t["name"].toString());
        if (t["name"].toString() == keep) select = idx;
    }
    tabs_->setCurrentIndex(select);
    tabs_->blockSignals(false);
}

bool AttributeEditor::eventFilter(QObject* o, QEvent* e) {
    if (e->type() == QEvent::FocusOut) {
        auto* note = qobject_cast<QPlainTextEdit*>(o);
        if (note && note->property("editing").toBool()) {
            note->setProperty("editing", false);
            if (!note->property("notesHad").toBool()) {
                exec("addAttr", QJsonObject{{"args", QJsonArray{note->property("notesNode").toString()}}, {"longName", "notes"}, {"dataType", "string"}});
                note->setProperty("notesHad", true);
            }
            set_plug(note->property("notesPlug").toString(), note->toPlainText(), "string");
        }
    }
    return Panel::eventFilter(o, e);
}

} // namespace az
