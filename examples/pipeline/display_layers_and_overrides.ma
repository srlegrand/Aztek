//Maya ASCII 2024 scene
//Name: display_layers_and_overrides.ma
//Codeset: UTF-8
requires maya "2024";
currentUnit -l centimeter -a degree -t film;
fileInfo "application" "aztek";
fileInfo "version" "1.0.0";
createNode transform -s -n "persp";
	setAttr ".v" no;
	setAttr ".t" -type "double3" 6 8 12 ;
	setAttr ".r" -type "double3" -29.205932247399417 26.565051177077994 0 ;
createNode camera -s -n "perspShape" -p "persp";
	setAttr ".coi" 15.370426148939398;
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
createNode transform -n "props0";
	setAttr ".t" -type "double3" -2.4000000000000004 0.5 0 ;
createNode mesh -n "propsShape0" -p "props0";
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
	setAttr ".vt[0:7]"  -0.5 -0.5 0.5 0.5 -0.5 0.5 0.5 0.5 0.5 -0.5 0.5 0.5 0.5 0.5 -0.5 -0.5 0.5 -0.5 0.5 -0.5 -0.5 -0.5 -0.5 -0.5;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "props1";
	setAttr ".t" -type "double3" -0.8 0.5 0 ;
createNode mesh -n "propsShape1" -p "props1";
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
	setAttr ".vt[0:7]"  -0.5 -0.7 0.5 0.5 -0.7 0.5 0.5 0.7 0.5 -0.5 0.7 0.5 0.5 0.7 -0.5 -0.5 0.7 -0.5 0.5 -0.7 -0.5 -0.5 -0.7 -0.5;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "props2";
	setAttr ".t" -type "double3" 0.8 0.5 0 ;
createNode mesh -n "propsShape2" -p "props2";
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
	setAttr ".vt[0:7]"  -0.5 -0.9 0.5 0.5 -0.9 0.5 0.5 0.9 0.5 -0.5 0.9 0.5 0.5 0.9 -0.5 -0.5 0.9 -0.5 0.5 -0.9 -0.5 -0.5 -0.9 -0.5;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "props3";
	setAttr ".t" -type "double3" 2.4000000000000004 0.5 0 ;
createNode mesh -n "propsShape3" -p "props3";
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
	setAttr ".vt[0:7]"  -0.5 -1.1 0.5 0.5 -1.1 0.5 0.5 1.1 0.5 -0.5 1.1 0.5 0.5 1.1 -0.5 -0.5 1.1 -0.5 0.5 -1.1 -0.5 -0.5 -1.1 -0.5;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "set0";
	setAttr ".t" -type "double3" -2.4000000000000004 0.5 -3 ;
createNode mesh -n "setShape0" -p "set0";
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
	setAttr ".vt[0:7]"  -0.5 -0.5 0.5 0.5 -0.5 0.5 0.5 0.5 0.5 -0.5 0.5 0.5 0.5 0.5 -0.5 -0.5 0.5 -0.5 0.5 -0.5 -0.5 -0.5 -0.5 -0.5;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "set1";
	setAttr ".t" -type "double3" -0.8 0.5 -3 ;
createNode mesh -n "setShape1" -p "set1";
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
	setAttr ".vt[0:7]"  -0.5 -0.7 0.5 0.5 -0.7 0.5 0.5 0.7 0.5 -0.5 0.7 0.5 0.5 0.7 -0.5 -0.5 0.7 -0.5 0.5 -0.7 -0.5 -0.5 -0.7 -0.5;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "set2";
	setAttr ".t" -type "double3" 0.8 0.5 -3 ;
createNode mesh -n "setShape2" -p "set2";
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
	setAttr ".vt[0:7]"  -0.5 -0.9 0.5 0.5 -0.9 0.5 0.5 0.9 0.5 -0.5 0.9 0.5 0.5 0.9 -0.5 -0.5 0.9 -0.5 0.5 -0.9 -0.5 -0.5 -0.9 -0.5;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "set3";
	setAttr ".t" -type "double3" 2.4000000000000004 0.5 -3 ;
createNode mesh -n "setShape3" -p "set3";
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
	setAttr ".vt[0:7]"  -0.5 -1.1 0.5 0.5 -1.1 0.5 0.5 1.1 0.5 -0.5 1.1 0.5 0.5 1.1 -0.5 -0.5 1.1 -0.5 0.5 -1.1 -0.5 -0.5 -1.1 -0.5;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "guides0";
	setAttr ".t" -type "double3" -2.4000000000000004 0.5 3 ;
createNode mesh -n "guidesShape0" -p "guides0";
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
	setAttr ".vt[0:7]"  -0.5 -0.5 0.5 0.5 -0.5 0.5 0.5 0.5 0.5 -0.5 0.5 0.5 0.5 0.5 -0.5 -0.5 0.5 -0.5 0.5 -0.5 -0.5 -0.5 -0.5 -0.5;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "guides1";
	setAttr ".t" -type "double3" -0.8 0.5 3 ;
createNode mesh -n "guidesShape1" -p "guides1";
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
	setAttr ".vt[0:7]"  -0.5 -0.7 0.5 0.5 -0.7 0.5 0.5 0.7 0.5 -0.5 0.7 0.5 0.5 0.7 -0.5 -0.5 0.7 -0.5 0.5 -0.7 -0.5 -0.5 -0.7 -0.5;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "guides2";
	setAttr ".t" -type "double3" 0.8 0.5 3 ;
createNode mesh -n "guidesShape2" -p "guides2";
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
	setAttr ".vt[0:7]"  -0.5 -0.9 0.5 0.5 -0.9 0.5 0.5 0.9 0.5 -0.5 0.9 0.5 0.5 0.9 -0.5 -0.5 0.9 -0.5 0.5 -0.9 -0.5 -0.5 -0.9 -0.5;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "guides3";
	setAttr ".t" -type "double3" 2.4000000000000004 0.5 3 ;
createNode mesh -n "guidesShape3" -p "guides3";
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
	setAttr ".vt[0:7]"  -0.5 -1.1 0.5 0.5 -1.1 0.5 0.5 1.1 0.5 -0.5 1.1 0.5 0.5 1.1 -0.5 -0.5 1.1 -0.5 0.5 -1.1 -0.5 -0.5 -1.1 -0.5;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "coloredCtrl";
	setAttr ".t" -type "double3" 5 0 0 ;
createNode nurbsCurve -n "coloredCtrlShape" -p "coloredCtrl";
	setAttr -k off ".v";
	setAttr ".ove" yes;
	setAttr ".ovrgbf" yes;
	setAttr ".ovrgb" -type "float3" 1 0.4 0.8 ;
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
createNode makeNurbCircle -n "makeNurbCircle1";
	setAttr ".normal" -type "double3" 0 1 0 ;
	setAttr ".radius" 1.2;
createNode lambert -n "floorMat";
	setAttr ".c" -type "float3" 0.45 0.45 0.45 ;
createNode shadingEngine -n "floorMatSG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode displayLayer -n "propsLayer";
	setAttr ".do" 0;
	setAttr ".c" 13;
createNode displayLayer -n "setLayer";
	setAttr ".dt" 2;
	setAttr ".do" 1;
	setAttr ".c" 6;
createNode displayLayer -n "guidesLayer";
	setAttr ".dt" 1;
	setAttr ".do" 2;
	setAttr ".c" 17;
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
	setAttr ".b" -type "string" "playbackOptions -min 1 -max 24 -ast 1 -aet 240 ";
	setAttr ".st" 6;
select -ne :time1;
	setAttr ".o" 1;
	setAttr ".unw" 1;
select -ne :initialShadingGroup;
	setAttr -s 12 ".dsm";
	setAttr ".ro" yes;
relationship "link" ":lightLinker1" ":initialShadingGroup.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "floorMatSG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" ":initialShadingGroup.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "floorMatSG.message" ":defaultLightSet.message";
connectAttr "floorMat.oc" "floorMatSG.ss";
connectAttr "guidesLayer.di" "guides0.do";
connectAttr "guidesLayer.di" "guides1.do";
connectAttr "guidesLayer.di" "guides2.do";
connectAttr "guidesLayer.di" "guides3.do";
connectAttr "makeNurbCircle1.outputCurve" "coloredCtrlShape.create";
connectAttr "propsLayer.di" "props0.do";
connectAttr "propsLayer.di" "props1.do";
connectAttr "propsLayer.di" "props2.do";
connectAttr "propsLayer.di" "props3.do";
connectAttr "setLayer.di" "set0.do";
connectAttr "setLayer.di" "set1.do";
connectAttr "setLayer.di" "set2.do";
connectAttr "setLayer.di" "set3.do";
connectAttr "fillLight.iog" ":defaultLightSet.dsm" -na;
connectAttr "fillLightShape.ltd" ":lightList1.l" -na;
connectAttr "floorMat.msg" ":defaultShaderList1.s" -na;
connectAttr "floorMatSG.pa" ":renderPartition.st" -na;
connectAttr "floorShape.iog" "floorMatSG.dsm" -na;
connectAttr "guidesShape0.iog" ":initialShadingGroup.dsm" -na;
connectAttr "guidesShape1.iog" ":initialShadingGroup.dsm" -na;
connectAttr "guidesShape2.iog" ":initialShadingGroup.dsm" -na;
connectAttr "guidesShape3.iog" ":initialShadingGroup.dsm" -na;
connectAttr "keyLight.iog" ":defaultLightSet.dsm" -na;
connectAttr "keyLightShape.ltd" ":lightList1.l" -na;
connectAttr "propsShape0.iog" ":initialShadingGroup.dsm" -na;
connectAttr "propsShape1.iog" ":initialShadingGroup.dsm" -na;
connectAttr "propsShape2.iog" ":initialShadingGroup.dsm" -na;
connectAttr "propsShape3.iog" ":initialShadingGroup.dsm" -na;
connectAttr "setShape0.iog" ":initialShadingGroup.dsm" -na;
connectAttr "setShape1.iog" ":initialShadingGroup.dsm" -na;
connectAttr "setShape2.iog" ":initialShadingGroup.dsm" -na;
connectAttr "setShape3.iog" ":initialShadingGroup.dsm" -na;
// End of display_layers_and_overrides.ma
