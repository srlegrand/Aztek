//Maya ASCII 2024 scene
// Written for the tests in the form the reference writes a plug-in deformer on a curve: the original shape, groupParts,
// groupId, tweak and the deformer's set. mGear makes its spine and neck curves this way.
requires maya "2024";
requires -nodeType "mgear_curveCns" "mgear_solvers" "4.2.2";
currentUnit -l centimeter -a degree -t film;
createNode transform -n "a";
	setAttr ".t" -type "double3" 0 0 0 ;
createNode transform -n "b";
	setAttr ".t" -type "double3" 5 0 0 ;
createNode transform -n "c";
	setAttr ".t" -type "double3" 10 0 0 ;
createNode transform -n "crv";
createNode nurbsCurve -n "crvShape" -p "crv";
	setAttr -k off ".v";
	setAttr -s 3 ".cp";
	setAttr ".cc" -type "nurbsCurve"
		1 2 0 no 3
		3 0 1 2
		3
		0 0 0
		5 0 0
		10 0 0
		;
createNode nurbsCurve -n "crvShapeOrig" -p "crv";
	setAttr -k off ".v";
	setAttr ".io" yes;
	setAttr ".cc" -type "nurbsCurve"
		1 2 0 no 3
		3 0 1 2
		3
		0 0 0
		0 0 0
		0 0 0
		;
createNode mgear_curveCns -n "mgear_curveCns1";
	setAttr -s 3 ".inputs";
createNode objectSet -n "mgear_curveCns1Set";
	setAttr ".ihi" 0;
	setAttr ".vo" yes;
createNode groupId -n "mgear_curveCns1GroupId";
	setAttr ".ihi" 0;
createNode groupParts -n "mgear_curveCns1GroupParts";
	setAttr ".ihi" 0;
	setAttr ".ic" -type "componentList" 1 "cv[*]";
createNode tweak -n "tweak1";
createNode objectSet -n "tweakSet1";
	setAttr ".ihi" 0;
	setAttr ".vo" yes;
createNode groupId -n "groupId2";
	setAttr ".ihi" 0;
createNode groupParts -n "groupParts2";
	setAttr ".ihi" 0;
	setAttr ".ic" -type "componentList" 1 "cv[*]";
connectAttr "a.wm" "mgear_curveCns1.inputs[0]";
connectAttr "b.wm" "mgear_curveCns1.inputs[1]";
connectAttr "c.wm" "mgear_curveCns1.inputs[2]";
connectAttr "mgear_curveCns1GroupId.id" "crvShape.iog.og[0].gid";
connectAttr "mgear_curveCns1Set.mwc" "crvShape.iog.og[0].gco";
connectAttr "groupId2.id" "crvShape.iog.og[1].gid";
connectAttr "tweakSet1.mwc" "crvShape.iog.og[1].gco";
connectAttr "mgear_curveCns1.og[0]" "crvShape.cr";
connectAttr "tweak1.pl[0].cp[0]" "crvShape.twl";
connectAttr "mgear_curveCns1GroupParts.og" "mgear_curveCns1.ip[0].ig";
connectAttr "mgear_curveCns1GroupId.id" "mgear_curveCns1.ip[0].gi";
connectAttr "mgear_curveCns1GroupId.msg" "mgear_curveCns1Set.gn" -na;
connectAttr "crvShape.iog.og[0]" "mgear_curveCns1Set.dsm" -na;
connectAttr "mgear_curveCns1.msg" "mgear_curveCns1Set.ub[0]";
connectAttr "tweak1.og[0]" "mgear_curveCns1GroupParts.ig";
connectAttr "mgear_curveCns1GroupId.id" "mgear_curveCns1GroupParts.gi";
connectAttr "groupParts2.og" "tweak1.ip[0].ig";
connectAttr "groupId2.id" "tweak1.ip[0].gi";
connectAttr "groupId2.msg" "tweakSet1.gn" -na;
connectAttr "crvShape.iog.og[1]" "tweakSet1.dsm" -na;
connectAttr "tweak1.msg" "tweakSet1.ub[0]";
connectAttr "crvShapeOrig.ws" "groupParts2.ig";
connectAttr "groupId2.id" "groupParts2.gi";
