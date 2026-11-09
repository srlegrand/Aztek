//Maya ASCII 2024 scene
//Name: smooth_preview_creases.ma
//Codeset: UTF-8
requires maya "2024";
currentUnit -l centimeter -a degree -t film;
fileInfo "application" "aztek";
fileInfo "version" "1.0.0";
createNode transform -s -n "persp";
	setAttr ".v" no;
	setAttr ".t" -type "double3" 0 5 11 ;
	setAttr ".r" -type "double3" -19.983106521899977 0 0 ;
createNode camera -s -n "perspShape" -p "persp";
	setAttr ".coi" 11.704699910719626;
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
createNode transform -n "cage0";
	setAttr ".t" -type "double3" -3.5 1 0 ;
createNode mesh -n "cageShape0" -p "cage0";
	setAttr -k off ".v";
	setAttr ".displaySmoothMesh" 2;
	setAttr ".smoothLevel" 1;
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
	setAttr ".vt[0:7]"  -1 -1 1 1 -1 1 1 1 1 -1 1 1 1 1 -1 -1 1 -1 1 -1 -1 -1 -1 -1;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "cage1";
	setAttr ".t" -type "double3" 0 1 0 ;
createNode mesh -n "cageShape1" -p "cage1";
	setAttr -k off ".v";
	setAttr ".displaySmoothMesh" 2;
	setAttr ".smoothLevel" 1;
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
	setAttr ".vt[0:7]"  -1 -1 1 1 -1 1 1 1 1 -1 1 1 1 1 -1 -1 1 -1 1 -1 -1 -1 -1 -1;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "cage2";
	setAttr ".t" -type "double3" 3.5 1 0 ;
createNode mesh -n "cageShape2" -p "cage2";
	setAttr -k off ".v";
	setAttr ".displaySmoothMesh" 2;
	setAttr ".smoothLevel" 1;
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
	setAttr ".vt[0:7]"  -1 -1 1 1 -1 1 1 1 1 -1 1 1 1 1 -1 -1 1 -1 1 -1 -1 -1 -1 -1;
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
	setAttr ".vt[0:3]"  -8 0 8 8 0 8 -8 0 -8 8 0 -8;
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
createNode blinn -n "cageMat";
	setAttr ".c" -type "float3" 0.9 0.85 0.75 ;
createNode shadingEngine -n "cageMatSG";
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
	setAttr ".b" -type "string" "playbackOptions -min 1 -max 120 -ast 1 -aet 200 ";
	setAttr ".st" 6;
select -ne :time1;
	setAttr ".o" 1;
	setAttr ".unw" 1;
relationship "link" ":lightLinker1" ":initialShadingGroup.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "cageMatSG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "floorMatSG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" ":initialShadingGroup.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "cageMatSG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "floorMatSG.message" ":defaultLightSet.message";
connectAttr "cageMat.oc" "cageMatSG.ss";
connectAttr "floorMat.oc" "floorMatSG.ss";
connectAttr "cageMat.msg" ":defaultShaderList1.s" -na;
connectAttr "cageMatSG.pa" ":renderPartition.st" -na;
connectAttr "cageShape0.iog" "cageMatSG.dsm" -na;
connectAttr "cageShape1.iog" "cageMatSG.dsm" -na;
connectAttr "cageShape2.iog" "cageMatSG.dsm" -na;
connectAttr "fillLight.iog" ":defaultLightSet.dsm" -na;
connectAttr "fillLightShape.ltd" ":lightList1.l" -na;
connectAttr "floorMat.msg" ":defaultShaderList1.s" -na;
connectAttr "floorMatSG.pa" ":renderPartition.st" -na;
connectAttr "floorShape.iog" "floorMatSG.dsm" -na;
connectAttr "keyLight.iog" ":defaultLightSet.dsm" -na;
connectAttr "keyLightShape.ltd" ":lightList1.l" -na;
// End of smooth_preview_creases.ma
