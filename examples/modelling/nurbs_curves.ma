//Maya ASCII 2024 scene
//Name: nurbs_curves.ma
//Codeset: UTF-8
requires maya "2024";
currentUnit -l centimeter -a degree -t film;
fileInfo "application" "aztek";
fileInfo "version" "1.0.0";
createNode transform -s -n "persp";
	setAttr ".v" no;
	setAttr ".t" -type "double3" 0 14 9 ;
	setAttr ".r" -type "double3" -57.2647737278924 0 0 ;
createNode camera -s -n "perspShape" -p "persp";
	setAttr ".coi" 16.64331697709324;
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
createNode transform -n "cvCurve";
createNode nurbsCurve -n "cvCurveShape" -p "cvCurve";
	setAttr -k off ".v";
	setAttr ".cc" -type "nurbsCurve" 
		3 1 0 no 3
		6 0 0 0 1 1 1
		4
		-6 0 0
		-5 0 2
		-3 0 -1
		-2 0 2
		;
createNode transform -n "epCurve";
createNode nurbsCurve -n "epCurveShape" -p "epCurve";
	setAttr -k off ".v";
	setAttr ".cc" -type "nurbsCurve" 
		3 1 0 no 3
		6 0 0 0 1 1 1
		4
		-1 0 0
		0.0000000000000003747002708109903 0 6.666666666666666
		0.9999999999999998 0 -4.666666666666667
		2 0 2
		;
createNode transform -n "circle";
	setAttr ".t" -type "double3" 5 0 1 ;
createNode nurbsCurve -n "circleShape" -p "circle";
	setAttr -k off ".v";
	setAttr ".cc" -type "nurbsCurve" 
		3 8 2 no 3
		13 -2 -1 0 1 2 3 4 5 6 7 8 9 10
		11
		0.9403339498694692 0 -0.9403339498694689
		0.00000000000000008142878787733095 0 -1.3298330250652652
		-0.9403339498694689 0 -0.9403339498694692
		-1.3298330250652652 0 -0.0000000000000001628575757546619
		-0.9403339498694693 0 0.9403339498694689
		-0.00000000000000024428636363199285 0 1.3298330250652652
		0.9403339498694688 0 0.9403339498694693
		1.3298330250652652 0 0.0000000000000003257151515093238
		0.9403339498694692 0 -0.9403339498694689
		0.00000000000000008142878787733095 0 -1.3298330250652652
		-0.9403339498694689 0 -0.9403339498694692
		;
createNode transform -n "arc";
	setAttr ".t" -type "double3" 5 0 -3 ;
createNode nurbsCurve -n "arcShape" -p "arc";
	setAttr -k off ".v";
	setAttr ".cc" -type "nurbsCurve" 
		3 8 0 no 3
		13 0 0 0 1 2 3 4 5 6 7 8 8 8
		11
		1.2 0 0
		1.2033914271945676 0 -0.2805480480978149
		1.046135652838111 0 -0.6952761033465157
		0.6125019343723013 0 -1.060614971727152
		0.21266241261990704 0 -1.2058783105513695
		-0.21266443212544206 0 -1.2060799275923462
		-0.6122719724655489 0 -1.0604201211466575
		-0.938315232590217 0 -0.7871640630457958
		-1.2208443858305087 0 -0.29554635780171945
		-1.2267711276719098 0 0.14795517783256082
		-1.12763114494309 0 0.4104241719908024
		;
createNode transform -n "cvOffset";
createNode nurbsCurve -n "cvOffsetShape" -p "cvOffset";
	setAttr -k off ".v";
	setAttr ".cc" -type "nurbsCurve" 
		3 1 0 no 3
		6 0 0 0 1 1 1
		4
		-5.642254173787226 0 -0.1789355298070769
		-4.344631847357778 0 0.8152938022832336
		-1.8629729344808665 0 -0.5559898445966053
		-1.6205380677351302 0 1.8734747378512442
		;
createNode transform -n "epRebuilt";
createNode nurbsCurve -n "epRebuiltShape" -p "epRebuilt";
	setAttr -k off ".v";
	setAttr ".cc" -type "nurbsCurve" 
		3 12 0 no 3
		17 0 0 0 1 2 3 4 5 6 7 8 9 10 11 12 12 12
		15
		-1 0 0
		-0.9393201172805752 0 0.34850828708374
		-0.8655504079862647 0 0.9338926764005814
		-0.6622343703431813 0 1.5637434587727848
		-0.5403376445775668 0 2.239991308876164
		0.1274016235554745 0 2.00591733041686
		0.2671780876810267 0 1.4845856199638228
		0.4999999999999997 0 0.9999999999999998
		0.732821912318974 0 0.5154143800361762
		0.8725983764445249 0 -0.005917330416861712
		1.540337644577567 0 -0.2399913088761618
		1.6622343703431814 0 0.4362565412272151
		1.8655504079862657 0 1.0661073235994196
		1.9393201172805752 0 1.6514917129162598
		2 0 2
		;
createNode transform -n "attachA";
createNode nurbsCurve -n "attachAShape" -p "attachA";
	setAttr -k off ".v";
	setAttr ".cc" -type "nurbsCurve" 
		1 1 0 no 3
		2 0 1
		2
		-6 0 -4
		-4 0 -4
		;
createNode transform -n "attachB";
createNode nurbsCurve -n "attachBShape" -p "attachB";
	setAttr -k off ".v";
	setAttr ".cc" -type "nurbsCurve" 
		1 1 0 no 3
		2 0 1
		2
		-4 0 -4
		-3 0 -2
		;
createNode transform -n "attached";
createNode nurbsCurve -n "attachedShape" -p "attached";
	setAttr -k off ".v";
	setAttr ".cc" -type "nurbsCurve" 
		1 3 0 no 3
		4 0 1 2 3
		4
		-6 0 -4
		-4.587977340833405 0 -4
		-3.6314757303333054 0 -3.262951460666611
		-3 0 -2
		;
createNode makeNurbCircle -n "makeNurbCircle1";
	setAttr ".normal" -type "double3" 0 1 0 ;
	setAttr ".radius" 1.2;
createNode makeNurbCircle -n "makeNurbCircle2";
	setAttr ".normal" -type "double3" 0 1 0 ;
	setAttr ".radius" 1.2;
	setAttr ".sweep" 200;
createNode aiOptions -s -n "defaultArnoldRenderOptions";
createNode aiAOVFilter -s -n "defaultArnoldFilter";
createNode aiAOVDriver -s -n "defaultArnoldDriver";
createNode aiAOVDriver -s -n "defaultArnoldDisplayDriver";
createNode script -n "sceneConfigurationScriptNode";
	setAttr ".b" -type "string" "playbackOptions -min 1 -max 120 -ast 1 -aet 200 ";
	setAttr ".st" 6;
select -ne :time1;
	setAttr ".o" 1;
	setAttr ".unw" 1;
connectAttr "makeNurbCircle1.outputCurve" "circleShape.create";
connectAttr "makeNurbCircle2.outputCurve" "arcShape.create";
// End of nurbs_curves.ma
