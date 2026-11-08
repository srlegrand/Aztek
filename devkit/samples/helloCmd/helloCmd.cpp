// Sample command: azHello -name n -count c makes c locators named n, undoably, and returns their names.
//   loadPlugin helloCmd; azHello -n "marker" -c 3; undo; redo;
#include <maya/MArgDatabase.h>
#include <maya/MDagModifier.h>
#include <maya/MFnDependencyNode.h>
#include <maya/MFnPlugin.h>
#include <maya/MGlobal.h>
#include <maya/MPxCommand.h>
#include <maya/MSyntax.h>

class HelloCmd : public MPxCommand {
public:
    static void* creator() { return new HelloCmd; }

    static MSyntax newSyntax() {
        MSyntax s;
        s.addFlag("-n", "-name", MSyntax::kString);
        s.addFlag("-c", "-count", MSyntax::kLong);
        s.addFlag("-v", "-verbose");
        return s;
    }

    MStatus doIt(const MArgList& args) override {
        MStatus st;
        MArgDatabase db(syntax(), args, &st);
        if (!st) return st;
        name_ = "hello";
        count_ = 1;
        if (db.isFlagSet("-name")) db.getFlagArgument("-name", 0, name_);
        if (db.isFlagSet("-count")) db.getFlagArgument("-count", 0, count_);
        if (count_ < 1) {
            displayError("azHello: -count must be at least 1");
            return MS::kInvalidParameter;
        }
        verbose_ = db.isFlagSet("-verbose");
        return redoIt();
    }

    MStatus redoIt() override {
        mod_ = MDagModifier();
        made_.clear();
        for (int i = 0; i < count_; ++i) {
            MStatus st;
            MObject loc = mod_.createNode("locator", MObject::kNullObj, &st);
            if (!st) return st;
            MString n = name_;
            n += (i + 1);
            mod_.renameNode(loc, n);
            made_.append(loc);
        }
        MStatus st = mod_.doIt();
        if (!st) return st;
        clearResult();
        for (unsigned int i = 0; i < made_.length(); ++i) {
            MString n = MFnDependencyNode(made_[i]).name();
            appendToResult(n);
            if (verbose_) MGlobal::displayInfo("azHello made " + n);
        }
        return MS::kSuccess;
    }

    MStatus undoIt() override { return mod_.undoIt(); }

    bool isUndoable() const override { return true; }

private:
    MString name_;
    int count_ = 1;
    bool verbose_ = false;
    MDagModifier mod_;
    MObjectArray made_;
};

MStatus initializePlugin(MObject obj) {
    MFnPlugin plugin(obj, "Aztek samples", "1.0", "Any");
    return plugin.registerCommand("azHello", HelloCmd::creator, HelloCmd::newSyntax);
}

MStatus uninitializePlugin(MObject obj) {
    MFnPlugin plugin(obj);
    return plugin.deregisterCommand("azHello");
}
