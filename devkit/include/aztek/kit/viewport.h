// Aztek plug-in kit: locator nodes and their drawing.
//
// A locator node from a plug-in is a shape in the scene and computes like any node. Its Viewport 2.0 draw override is
// created and asked for what it draws (prepareForDraw, addUIDrawables); the lines, circles, cones and text it adds are
// collected for the host's viewport. The legacy draw through M3dView is not called.
#pragma once
#include "plugin.h"
#include "messages.h"

class M3dView {
public:
    enum DisplayStyle { kBoundingBox, kFlatShaded, kGouraudShaded, kWireFrame, kPoints };
    enum DisplayStatus { kActive, kLive, kDormant, kInvisible, kHilite, kTemplate, kActiveTemplate, kActiveComponent, kLead, kIntermediateObject, kActiveAffected, kNoStatus };
    enum TextPosition { kLeft, kCenter, kRight };
    enum ColorTable { kActiveColors, kDormantColors, kTemplateColor, kBackgroundColor };
    M3dView();
    static M3dView active3dView(MStatus* status = 0);
    MStatus beginGL();
    MStatus endGL();
    void setDrawColor(const MColor& color);
    void setDrawColor(unsigned int index, ColorTable table = kActiveColors);
    MStatus drawText(const MString& text, const MPoint& position, TextPosition align = kLeft);
    MStatus modelViewMatrix(MMatrix& out) const;
    MStatus projectionMatrix(MMatrix& out) const;
    MStatus refresh(bool all = false, bool force = false);
    int portWidth(MStatus* status = 0);
    int portHeight(MStatus* status = 0);
};

class MPxLocatorNode : public MPxNode {
public:
    MPxLocatorNode();
    virtual ~MPxLocatorNode();
    virtual Type type() const;
    virtual void draw(M3dView& view, const MDagPath& path, M3dView::DisplayStyle style, M3dView::DisplayStatus status);
    virtual bool isBounded() const;
    virtual MBoundingBox boundingBox() const;
    virtual bool isTransparent() const;
    virtual bool drawLast() const;
    virtual bool excludeAsLocator() const;
    MColor color(M3dView::DisplayStatus status);

    static MObject underWorldObject;
    static MObject localPosition;
    static MObject localPositionX;
    static MObject localPositionY;
    static MObject localPositionZ;
    static MObject worldPosition;
    static MObject worldPositionX;
    static MObject worldPositionY;
    static MObject worldPositionZ;
    static MObject localScale;
    static MObject localScaleX;
    static MObject localScaleY;
    static MObject localScaleZ;
    static MObject nodeBoundingBox;
};

class MUserData {
public:
    MUserData();
    MUserData(bool deleteAfterUse);
    virtual ~MUserData();
    bool deleteAfterUse() const;
    void setDeleteAfterUse(bool on);

private:
    bool delete_;
};

namespace MHWRender {

enum DrawAPI { kNone = 0, kOpenGL = 1 << 0, kDirectX11 = 1 << 1, kOpenGLCoreProfile = 1 << 2, kAllDevices = kOpenGL | kDirectX11 | kOpenGLCoreProfile };
inline DrawAPI operator|(DrawAPI a, DrawAPI b) { return (DrawAPI)((int)a | (int)b); }

enum DisplayStatus { kActive, kLive, kDormant, kInvisible, kHilite, kTemplate, kActiveTemplate, kActiveComponent, kLead, kIntermediateObject, kActiveAffected, kNoStatus };

class MFrameContext {
public:
    MFrameContext();
    virtual ~MFrameContext();
    MDagPath getCurrentCameraPath(MStatus* status = 0) const;
    void getViewportDimensions(int& originX, int& originY, int& width, int& height) const;
};

class MDrawContext : public MFrameContext {
public:
    MDrawContext();
};

// What a draw override draws: kept as shapes in object space, each with the colour set before it.
class MUIDrawManager {
public:
    enum TextAlignment { kLeft, kCenter, kRight };
    enum LineStyle { kSolid, kShortDotted, kShortDashed, kDashed, kDotted };
    enum Primitive { kPoints, kLines, kLineStrip, kClosedLine, kTriangles, kTriStrip };
    MUIDrawManager();
    void beginDrawable();
    void endDrawable();
    void setColor(const MColor& color);
    void setLineWidth(float width);
    void setLineStyle(LineStyle style);
    void setPointSize(float size);
    void setFontSize(unsigned int size);
    void line(const MPoint& a, const MPoint& b);
    void point(const MPoint& p);
    void circle(const MPoint& center, const MVector& normal, double radius, bool filled = false);
    void rect(const MPoint& center, const MVector& up, const MVector& normal, double scaleX, double scaleY, bool filled = false);
    void sphere(const MPoint& center, double radius, bool filled = false);
    void cone(const MPoint& base, const MVector& direction, double radius, double height, bool filled = false);
    void box(const MPoint& center, const MVector& up, const MVector& right, double scaleX = 1.0, double scaleY = 1.0, double scaleZ = 1.0, bool filled = false);
    void text(const MPoint& position, const MString& text, TextAlignment alignment = kLeft, const int* backgroundSize = 0, const MColor* backgroundColor = 0, bool dynamic = false);
    void mesh(Primitive mode, const MPointArray& position);

    // The kit's own: what was drawn, as the host reads it.
    struct Item {
        int kind;  // 0 line (a, b), 1 circle (a centre, b normal, r), 2 cone (a base, b direction, r, h), 3 text (a), 4 point, 5 sphere
        MColor color;
        MPoint a, b;
        double r, h;
        bool filled;
        std::string text;
    };
    std::vector<Item> azItems;

private:
    MColor color_;
};

typedef void (*GeometryDrawOverrideCb)(const MDrawContext& context, const MUserData* data);

class MPxDrawOverride {
public:
    MPxDrawOverride(const MObject& obj, GeometryDrawOverrideCb callback, bool isAlwaysDirty = true);
    virtual ~MPxDrawOverride();
    virtual DrawAPI supportedDrawAPIs() const;
    virtual bool isBounded(const MDagPath& objPath, const MDagPath& cameraPath) const;
    virtual MBoundingBox boundingBox(const MDagPath& objPath, const MDagPath& cameraPath) const;
    virtual bool disableInternalBoundingBoxDraw() const;
    virtual MUserData* prepareForDraw(const MDagPath& objPath, const MDagPath& cameraPath, const MFrameContext& frameContext, MUserData* oldData) = 0;
    virtual bool hasUIDrawables() const;
    virtual void addUIDrawables(const MDagPath& objPath, MUIDrawManager& drawManager, const MFrameContext& frameContext, const MUserData* data);
    virtual bool excludedFromPostEffects() const;
    virtual bool isTransparent() const;
};

typedef MPxDrawOverride* (*DrawOverrideCreator)(const MObject& obj);

class MDrawRegistry {
public:
    static MStatus registerDrawOverrideCreator(const MString& drawClassification, const MString& registrantId, DrawOverrideCreator creator);
    static MStatus deregisterDrawOverrideCreator(const MString& drawClassification, const MString& registrantId);
};

class MGeometryUtilities {
public:
    static DisplayStatus displayStatus(const MDagPath& path, MStatus* status = 0);
    static MColor wireframeColor(const MDagPath& path);
};

class MRenderer {
public:
    static void setGeometryDrawDirty(const MObject& obj, bool topologyChanged = true);
};

}  // namespace MHWRender

class MFnCamera : public MFnDagNode {
public:
    MFnCamera();
    MFnCamera(const MObject& o, MStatus* status = 0);
    MFnCamera(const MDagPath& path, MStatus* status = 0);
    virtual MFn::Type type() const;
    MPoint eyePoint(MSpace::Space space = MSpace::kObject, MStatus* status = 0) const;
    MVector viewDirection(MSpace::Space space = MSpace::kObject, MStatus* status = 0) const;
    MVector upDirection(MSpace::Space space = MSpace::kObject, MStatus* status = 0) const;
    MVector rightDirection(MSpace::Space space = MSpace::kObject, MStatus* status = 0) const;
    bool isOrtho(MStatus* status = 0) const;
    double focalLength(MStatus* status = 0) const;
};

