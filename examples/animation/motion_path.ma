//Maya ASCII 2024 scene
//Name: motion_path.ma
//Codeset: UTF-8
requires maya "2024";
currentUnit -l centimeter -a degree -t film;
fileInfo "application" "aztek";
fileInfo "version" "1.0.0";
createNode transform -s -n "persp";
	setAttr ".v" no;
	setAttr ".t" -type "double3" 0 18 20 ;
	setAttr ".r" -type "double3" -38.65980825409009 0 0 ;
createNode camera -s -n "perspShape" -p "persp";
	setAttr ".coi" 25.612496949731394;
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
createNode transform -n "flightPath";
createNode nurbsCurve -n "flightPathShape" -p "flightPath";
	setAttr -k off ".v";
	setAttr ".cc" -type "nurbsCurve" 
		3 5 0 no 3
		10 0 0 0 1 2 3 4 5 5 5
		8
		-8 1 0
		-5 4 6
		0 2 8
		5 5 4
		8 1 -2
		2 3 -8
		-6 2 -5
		-8 1 0
		;
createNode transform -n "dart";
	setAttr ".t" -type "double3" -8 1 0 ;
	setAttr ".r" -type "double3" -24.487043623963295 26.209875634924092 -0.8975390583057589 ;
createNode mesh -n "dartShape" -p "dart";
	setAttr -k off ".v";
	setAttr ".vir" yes;
	setAttr ".vif" yes;
	setAttr ".uvst[0].uvsn" -type "string" "map1";
	setAttr -s 12 ".uvst[0].uvsp";
	setAttr ".uvst[0].uvsp[0:11]" -type "float2" 0.13 0.4578461 0.13 0.04215391 0.49 0.25 0 0.5 0.33333334 0.5 0.16666667 1 0.33333334 0.5 0.6666667 0.5 0.5 1 0.6666667 0.5 1 0.5 0.8333334 1 ;
	setAttr ".cuvs" -type "string" "map1";
	setAttr ".dcc" -type "string" "Ambient+Diffuse";
	setAttr ".covm[0]"  0 1 1;
	setAttr ".cdvm[0]"  0 1 1;
	setAttr -s 4 ".vt";
	setAttr ".vt[0:3]"  0.5 -0.00000000000000006123233995736766 -1 -0.2499999999999999 0.4330127018922193 -1 -0.2500000000000002 -0.43301270189221924 -1 0 0.00000000000000006123233995736766 1;
	setAttr -s 6 ".ed";
	setAttr ".ed[0:5]"  2 1 0 1 0 0 0 2 0 1 3 1 3 0 1 2 3 1;
	setAttr -s 4 -ch 12 ".fc[0:3]" -type "polyFaces" 
		f 3 0 1 2 mu 0 3 0 1 2 
		f 3 -2 3 4 mu 0 3 3 4 5 
		f 3 -1 5 -4 mu 0 3 6 7 8 
		f 3 -3 -5 -6 mu 0 3 9 10 11 ;
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
	setAttr ".vt[0:3]"  -15 0 15 15 0 15 -15 0 -15 15 0 -15;
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
createNode motionPath -n "dartPath";
	setAttr ".follow" yes;
	setAttr ".frontAxis" 2;
	setAttr ".bank" yes;
createNode animCurveTL -n "dartPath_uValue";
	setAttr ".tan" 18;
	setAttr ".wgt" no;
	setAttr -s 2 ".ktv[0:1]"  1 0 150 5;
createNode blinn -n "dartMat";
	setAttr ".c" -type "float3" 0.95 0.75 0.1 ;
createNode shadingEngine -n "dartMatSG";
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
	setAttr ".b" -type "string" "playbackOptions -min 1 -max 150 -ast 1 -aet 200 ";
	setAttr ".st" 6;
select -ne :time1;
	setAttr ".o" 1;
	setAttr ".unw" 1;
relationship "link" ":lightLinker1" ":initialShadingGroup.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "dartMatSG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "floorMatSG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" ":initialShadingGroup.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "dartMatSG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "floorMatSG.message" ":defaultLightSet.message";
connectAttr "dartMat.oc" "dartMatSG.ss";
connectAttr "dartPath.allCoordinates" "dart.translate";
connectAttr "dartPath.rotate" "dart.rotate";
connectAttr "dartPath_uValue.output" "dartPath.uValue";
connectAttr "flightPathShape.worldSpace" "dartPath.geometryPath";
connectAttr "floorMat.oc" "floorMatSG.ss";
connectAttr "dartMat.msg" ":defaultShaderList1.s" -na;
connectAttr "dartMatSG.pa" ":renderPartition.st" -na;
connectAttr "dartShape.iog" "dartMatSG.dsm" -na;
connectAttr "fillLight.iog" ":defaultLightSet.dsm" -na;
connectAttr "fillLightShape.ltd" ":lightList1.l" -na;
connectAttr "floorMat.msg" ":defaultShaderList1.s" -na;
connectAttr "floorMatSG.pa" ":renderPartition.st" -na;
connectAttr "floorShape.iog" "floorMatSG.dsm" -na;
connectAttr "keyLight.iog" ":defaultLightSet.dsm" -na;
connectAttr "keyLightShape.ltd" ":lightList1.l" -na;
// End of motion_path.ma
