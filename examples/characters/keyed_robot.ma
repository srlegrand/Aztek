//Maya ASCII 2024 scene
//Name: keyed_robot.ma
//Codeset: UTF-8
requires maya "2024";
currentUnit -l centimeter -a degree -t film;
fileInfo "application" "aztek";
fileInfo "version" "1.0.0";
createNode transform -s -n "persp";
	setAttr ".v" no;
	setAttr ".t" -type "double3" 14 7 14 ;
	setAttr ".r" -type "double3" -12.246689727311349 60.25511870305778 0 ;
createNode camera -s -n "perspShape" -p "persp";
	setAttr ".coi" 16.5;
	setAttr ".imn" -type "string" "persp";
	setAttr ".den" -type "string" "persp_depth";
	setAttr ".man" -type "string" "persp_mask";
	setAttr ".hc" -type "string" "viewSet -p %camera";
createNode transform -s -n "top";
	setAttr ".v" no;
	setAttr ".t" -type "double3" 0 1000.1 0 ;
	setAttr ".r" -type "double3" -90 0 0 ;
createNode camera -s -n "topShape" -p "top";
	setAttr ".coi" 1000.1;
	setAttr ".o" yes;
	setAttr ".rnd" no;
	setAttr ".imn" -type "string" "top";
	setAttr ".den" -type "string" "top_depth";
	setAttr ".man" -type "string" "top_mask";
	setAttr ".hc" -type "string" "viewSet -t %camera";
createNode transform -s -n "front";
	setAttr ".v" no;
	setAttr ".t" -type "double3" 0 0 1000.1 ;
createNode camera -s -n "frontShape" -p "front";
	setAttr ".coi" 1000.1;
	setAttr ".o" yes;
	setAttr ".rnd" no;
	setAttr ".imn" -type "string" "front";
	setAttr ".den" -type "string" "front_depth";
	setAttr ".man" -type "string" "front_mask";
	setAttr ".hc" -type "string" "viewSet -f %camera";
createNode transform -s -n "side";
	setAttr ".v" no;
	setAttr ".t" -type "double3" 1000.1 0 0 ;
	setAttr ".r" -type "double3" 0 90 0 ;
createNode camera -s -n "sideShape" -p "side";
	setAttr ".coi" 1000.1;
	setAttr ".o" yes;
	setAttr ".rnd" no;
	setAttr ".imn" -type "string" "side";
	setAttr ".den" -type "string" "side_depth";
	setAttr ".man" -type "string" "side_mask";
	setAttr ".hc" -type "string" "viewSet -s %camera";
createNode transform -n "torso";
	setAttr ".t" -type "double3" 0 4.6 0 ;
createNode mesh -n "torsoShape" -p "torso";
	setAttr -k off ".v";
	setAttr ".vir" yes;
	setAttr ".vif" yes;
	setAttr ".uvst[0].uvsn" -type "string" "map1";
	setAttr -s 14 ".uvst[0].uvsp";
	setAttr ".uvst[0].uvsp[0:13]" -type "float2" 0.375 0 0.625 0 0.625 0.25 0.375 0.25 0.625 0.5 0.375 0.5 0.625 0.75 0.375 0.75 0.625 1 0.375 1 0.875 0 0.875 0.25 0.125 0 0.125 0.25 ;
	setAttr ".cuvs" -type "string" "map1";
	setAttr ".dcc" -type "string" "Ambient+Diffuse";
	setAttr ".covm[0]"  0 1 1;
	setAttr ".cdvm[0]"  0 1 1;
	setAttr -s 8 ".vt";
	setAttr ".vt[0:7]"  -1 -1.2 0.6 1 -1.2 0.6 1 1.2 0.6 -1 1.2 0.6 1 1.2 -0.6 -1 1.2 -0.6 1 -1.2 -0.6 -1 -1.2 -0.6;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "head" -p "torso";
	setAttr ".t" -type "double3" 0 1.8000000000000007 0 ;
	setAttr ".rp" -type "double3" 0 -0.6000000000000005 0 ;
	setAttr ".sp" -type "double3" 0 -0.6000000000000005 0 ;
createNode mesh -n "headShape" -p "head";
	setAttr -k off ".v";
	setAttr ".vir" yes;
	setAttr ".vif" yes;
	setAttr ".uvst[0].uvsn" -type "string" "map1";
	setAttr -s 14 ".uvst[0].uvsp";
	setAttr ".uvst[0].uvsp[0:13]" -type "float2" 0.375 0 0.625 0 0.625 0.25 0.375 0.25 0.625 0.5 0.375 0.5 0.625 0.75 0.375 0.75 0.625 1 0.375 1 0.875 0 0.875 0.25 0.125 0 0.125 0.25 ;
	setAttr ".cuvs" -type "string" "map1";
	setAttr ".dcc" -type "string" "Ambient+Diffuse";
	setAttr ".covm[0]"  0 1 1;
	setAttr ".cdvm[0]"  0 1 1;
	setAttr -s 8 ".vt";
	setAttr ".vt[0:7]"  -0.6 -0.5 0.5 0.6 -0.5 0.5 0.6 0.5 0.5 -0.6 0.5 0.5 0.6 0.5 -0.5 -0.6 0.5 -0.5 0.6 -0.5 -0.5 -0.6 -0.5 -0.5;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "L_arm" -p "torso";
	setAttr ".t" -type "double3" 1.3 0 0 ;
	setAttr ".r" -type "double3" 30 0 0 ;
	setAttr ".rp" -type "double3" 0 1.1000000000000005 0 ;
	setAttr ".sp" -type "double3" 0 1.1000000000000005 0 ;
createNode mesh -n "L_armShape" -p "L_arm";
	setAttr -k off ".v";
	setAttr ".vir" yes;
	setAttr ".vif" yes;
	setAttr ".uvst[0].uvsn" -type "string" "map1";
	setAttr -s 14 ".uvst[0].uvsp";
	setAttr ".uvst[0].uvsp[0:13]" -type "float2" 0.375 0 0.625 0 0.625 0.25 0.375 0.25 0.625 0.5 0.375 0.5 0.625 0.75 0.375 0.75 0.625 1 0.375 1 0.875 0 0.875 0.25 0.125 0 0.125 0.25 ;
	setAttr ".cuvs" -type "string" "map1";
	setAttr ".dcc" -type "string" "Ambient+Diffuse";
	setAttr ".covm[0]"  0 1 1;
	setAttr ".cdvm[0]"  0 1 1;
	setAttr -s 8 ".vt";
	setAttr ".vt[0:7]"  -0.25 -1.1 0.25 0.25 -1.1 0.25 0.25 1.1 0.25 -0.25 1.1 0.25 0.25 1.1 -0.25 -0.25 1.1 -0.25 0.25 -1.1 -0.25 -0.25 -1.1 -0.25;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "L_leg" -p "torso";
	setAttr ".t" -type "double3" 0.52 -2.8 0 ;
	setAttr ".r" -type "double3" -25 0 0 ;
	setAttr ".rp" -type "double3" 0 1.5 0 ;
	setAttr ".sp" -type "double3" 0 1.5 0 ;
createNode mesh -n "L_legShape" -p "L_leg";
	setAttr -k off ".v";
	setAttr ".vir" yes;
	setAttr ".vif" yes;
	setAttr ".uvst[0].uvsn" -type "string" "map1";
	setAttr -s 14 ".uvst[0].uvsp";
	setAttr ".uvst[0].uvsp[0:13]" -type "float2" 0.375 0 0.625 0 0.625 0.25 0.375 0.25 0.625 0.5 0.375 0.5 0.625 0.75 0.375 0.75 0.625 1 0.375 1 0.875 0 0.875 0.25 0.125 0 0.125 0.25 ;
	setAttr ".cuvs" -type "string" "map1";
	setAttr ".dcc" -type "string" "Ambient+Diffuse";
	setAttr ".covm[0]"  0 1 1;
	setAttr ".cdvm[0]"  0 1 1;
	setAttr -s 8 ".vt";
	setAttr ".vt[0:7]"  -0.35 -1.5 0.35 0.35 -1.5 0.35 0.35 1.5 0.35 -0.35 1.5 0.35 0.35 1.5 -0.35 -0.35 1.5 -0.35 0.35 -1.5 -0.35 -0.35 -1.5 -0.35;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "R_arm" -p "torso";
	setAttr ".t" -type "double3" -1.3 0 0 ;
	setAttr ".r" -type "double3" -30 0 0 ;
	setAttr ".rp" -type "double3" 0 1.1000000000000005 0 ;
	setAttr ".sp" -type "double3" 0 1.1000000000000005 0 ;
createNode mesh -n "R_armShape" -p "R_arm";
	setAttr -k off ".v";
	setAttr ".vir" yes;
	setAttr ".vif" yes;
	setAttr ".uvst[0].uvsn" -type "string" "map1";
	setAttr -s 14 ".uvst[0].uvsp";
	setAttr ".uvst[0].uvsp[0:13]" -type "float2" 0.375 0 0.625 0 0.625 0.25 0.375 0.25 0.625 0.5 0.375 0.5 0.625 0.75 0.375 0.75 0.625 1 0.375 1 0.875 0 0.875 0.25 0.125 0 0.125 0.25 ;
	setAttr ".cuvs" -type "string" "map1";
	setAttr ".dcc" -type "string" "Ambient+Diffuse";
	setAttr ".covm[0]"  0 1 1;
	setAttr ".cdvm[0]"  0 1 1;
	setAttr -s 8 ".vt";
	setAttr ".vt[0:7]"  -0.25 -1.1 0.25 0.25 -1.1 0.25 0.25 1.1 0.25 -0.25 1.1 0.25 0.25 1.1 -0.25 -0.25 1.1 -0.25 0.25 -1.1 -0.25 -0.25 -1.1 -0.25;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "R_leg" -p "torso";
	setAttr ".t" -type "double3" -0.52 -2.8 0 ;
	setAttr ".r" -type "double3" 25 0 0 ;
	setAttr ".rp" -type "double3" 0 1.5 0 ;
	setAttr ".sp" -type "double3" 0 1.5 0 ;
createNode mesh -n "R_legShape" -p "R_leg";
	setAttr -k off ".v";
	setAttr ".vir" yes;
	setAttr ".vif" yes;
	setAttr ".uvst[0].uvsn" -type "string" "map1";
	setAttr -s 14 ".uvst[0].uvsp";
	setAttr ".uvst[0].uvsp[0:13]" -type "float2" 0.375 0 0.625 0 0.625 0.25 0.375 0.25 0.625 0.5 0.375 0.5 0.625 0.75 0.375 0.75 0.625 1 0.375 1 0.875 0 0.875 0.25 0.125 0 0.125 0.25 ;
	setAttr ".cuvs" -type "string" "map1";
	setAttr ".dcc" -type "string" "Ambient+Diffuse";
	setAttr ".covm[0]"  0 1 1;
	setAttr ".cdvm[0]"  0 1 1;
	setAttr -s 8 ".vt";
	setAttr ".vt[0:7]"  -0.35 -1.5 0.35 0.35 -1.5 0.35 0.35 1.5 0.35 -0.35 1.5 0.35 0.35 1.5 -0.35 -0.35 1.5 -0.35 0.35 -1.5 -0.35 -0.35 -1.5 -0.35;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "floor";
createNode mesh -n "floorShape" -p "floor";
	setAttr -k off ".v";
	setAttr ".vir" yes;
	setAttr ".vif" yes;
	setAttr ".uvst[0].uvsn" -type "string" "map1";
	setAttr -s 4 ".uvst[0].uvsp";
	setAttr ".uvst[0].uvsp[0:3]" -type "float2" 0 0 1 0 1 1 0 1 ;
	setAttr ".cuvs" -type "string" "map1";
	setAttr ".dcc" -type "string" "Ambient+Diffuse";
	setAttr ".covm[0]"  0 1 1;
	setAttr ".cdvm[0]"  0 1 1;
	setAttr -s 4 ".vt";
	setAttr ".vt[0:3]"  -20 0 20 20 0 20 -20 0 -20 20 0 -20;
	setAttr -s 4 ".ed";
	setAttr ".ed[0:3]"  0 1 1 1 3 1 3 2 1 2 0 1;
	setAttr -s 1 -ch 4 ".fc[0]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 ;
createNode transform -n "keyLight";
	setAttr ".r" -type "double3" -45 35 0 ;
createNode directionalLight -n "keyLightShape" -p "keyLight";
	setAttr -k off ".v";
createNode transform -n "fillLight";
createNode ambientLight -n "fillLightShape" -p "fillLight";
	setAttr -k off ".v";
	setAttr ".in" 0.25;
createNode animCurveTA -n "L_arm_rotateX";
	setAttr ".tan" 18;
	setAttr ".wgt" no;
	setAttr -s 3 ".ktv[0:2]"  1 30 13 -30 25 30;
	setAttr -s 3 ".kit[0:2]"  9 9 9;
	setAttr -s 3 ".kot[0:2]"  9 9 9;
	setAttr ".pre" 3;
	setAttr ".pst" 3;
createNode animCurveTA -n "L_leg_rotateX";
	setAttr ".tan" 18;
	setAttr ".wgt" no;
	setAttr -s 3 ".ktv[0:2]"  1 -25 13 25 25 -25;
	setAttr -s 3 ".kit[0:2]"  9 9 9;
	setAttr -s 3 ".kot[0:2]"  9 9 9;
	setAttr ".pre" 3;
	setAttr ".pst" 3;
createNode animCurveTA -n "R_arm_rotateX";
	setAttr ".tan" 18;
	setAttr ".wgt" no;
	setAttr -s 3 ".ktv[0:2]"  13 30 25 -30 37 30;
	setAttr -s 3 ".kit[0:2]"  9 9 9;
	setAttr -s 3 ".kot[0:2]"  9 9 9;
	setAttr ".pre" 3;
	setAttr ".pst" 3;
createNode animCurveTA -n "R_leg_rotateX";
	setAttr ".tan" 18;
	setAttr ".wgt" no;
	setAttr -s 3 ".ktv[0:2]"  13 -25 25 25 37 -25;
	setAttr -s 3 ".kit[0:2]"  9 9 9;
	setAttr -s 3 ".kot[0:2]"  9 9 9;
	setAttr ".pre" 3;
	setAttr ".pst" 3;
createNode animCurveTL -n "torso_translateY";
	setAttr ".tan" 18;
	setAttr ".wgt" no;
	setAttr -s 5 ".ktv[0:4]"  1 4.6 7 4.8 13 4.6 19 4.8 25 4.6;
	setAttr -s 5 ".kit[0:4]"  9 9 9 9 9;
	setAttr -s 5 ".kot[0:4]"  9 9 9 9 9;
	setAttr ".pre" 3;
	setAttr ".pst" 3;
createNode animCurveTL -n "torso_translateZ";
	setAttr ".tan" 18;
	setAttr ".wgt" no;
	setAttr -s 2 ".ktv[0:1]"  1 0 25 3;
	setAttr -s 2 ".kit[0:1]"  2 2;
	setAttr -s 2 ".kot[0:1]"  2 2;
	setAttr ".pre" 4;
	setAttr ".pst" 4;
createNode blinn -n "robotMat";
	setAttr ".c" -type "float3" 0.6 0.65 0.75 ;
createNode shadingEngine -n "robotMatSG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode lambert -n "floorMat";
	setAttr ".c" -type "float3" 0.45 0.45 0.45 ;
createNode shadingEngine -n "floorMatSG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode lightLinker -s -n "lightLinker1";
createNode aiOptions -s -n "defaultArnoldRenderOptions";
createNode aiAOVFilter -s -n "defaultArnoldFilter";
createNode aiAOVDriver -s -n "defaultArnoldDriver";
createNode aiAOVDriver -s -n "defaultArnoldDisplayDriver";
select -ne :lightList1;
	setAttr -s 2 ".l";
select -ne :defaultLightSet;
	setAttr -s 2 ".dsm";
createNode script -n "sceneConfigurationScriptNode";
	setAttr ".b" -type "string" "playbackOptions -min 1 -max 120 -ast 1 -aet 240 ";
	setAttr ".st" 6;
select -ne :time1;
	setAttr ".o" 1;
	setAttr ".unw" 1;
relationship "link" ":lightLinker1" ":initialShadingGroup.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "robotMatSG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "floorMatSG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" ":initialShadingGroup.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "robotMatSG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "floorMatSG.message" ":defaultLightSet.message";
connectAttr "L_arm_rotateX.o" "L_arm.rx";
connectAttr "L_leg_rotateX.o" "L_leg.rx";
connectAttr "R_arm_rotateX.o" "R_arm.rx";
connectAttr "R_leg_rotateX.o" "R_leg.rx";
connectAttr "floorMat.oc" "floorMatSG.ss";
connectAttr "robotMat.oc" "robotMatSG.ss";
connectAttr "torso_translateY.o" "torso.ty";
connectAttr "torso_translateZ.o" "torso.tz";
connectAttr "L_armShape.iog" "robotMatSG.dsm" -na;
connectAttr "L_legShape.iog" "robotMatSG.dsm" -na;
connectAttr "R_armShape.iog" "robotMatSG.dsm" -na;
connectAttr "R_legShape.iog" "robotMatSG.dsm" -na;
connectAttr "fillLight.iog" ":defaultLightSet.dsm" -na;
connectAttr "fillLightShape.ltd" ":lightList1.l" -na;
connectAttr "floorMat.msg" ":defaultShaderList1.s" -na;
connectAttr "floorMatSG.pa" ":renderPartition.st" -na;
connectAttr "floorShape.iog" "floorMatSG.dsm" -na;
connectAttr "headShape.iog" "robotMatSG.dsm" -na;
connectAttr "keyLight.iog" ":defaultLightSet.dsm" -na;
connectAttr "keyLightShape.ltd" ":lightList1.l" -na;
connectAttr "robotMat.msg" ":defaultShaderList1.s" -na;
connectAttr "robotMatSG.pa" ":renderPartition.st" -na;
connectAttr "torsoShape.iog" "robotMatSG.dsm" -na;
// End of keyed_robot.ma
