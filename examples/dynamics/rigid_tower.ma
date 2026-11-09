//Maya ASCII 2024 scene
//Name: rigid_tower.ma
//Codeset: UTF-8
requires maya "2024";
requires "aztekDynamics" "1.0";
currentUnit -l centimeter -a degree -t film;
fileInfo "application" "aztek";
fileInfo "version" "1.0.0";
createNode transform -s -n "persp";
	setAttr ".v" no;
	setAttr ".t" -type "double3" 800 500 900 ;
	setAttr ".r" -type "double3" -15.787048009751436 43.36342295838329 0 ;
createNode camera -s -n "perspShape" -p "persp";
	setAttr ".coi" 1286.4680330268607;
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
	setAttr ".vt[0:3]"  -2000 0 2000 2000 0 2000 -2000 0 -2000 2000 0 -2000;
	setAttr -s 4 ".ed";
	setAttr ".ed[0:3]"  0 1 1 1 3 1 3 2 1 2 0 1;
	setAttr -s 1 -ch 4 ".fc[0]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 ;
createNode transform -n "solver1";
createNode solver -n "solverShape1" -p "solver1";
	setAttr -k off ".v";
createNode transform -n "block_0_0";
	setAttr ".t" -type "double3" 0 22.5 -51 ;
createNode mesh -n "block_0_Shape0" -p "block_0_0";
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
	setAttr ".vt[0:7]"  -75 -22.5 24 75 -22.5 24 75 22.5 24 -75 22.5 24 75 22.5 -24 -75 22.5 -24 75 -22.5 -24 -75 -22.5 -24;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_0_1";
	setAttr ".t" -type "double3" 0 22.5 0 ;
createNode mesh -n "block_0_Shape1" -p "block_0_1";
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
	setAttr ".vt[0:7]"  -75 -22.5 24 75 -22.5 24 75 22.5 24 -75 22.5 24 75 22.5 -24 -75 22.5 -24 75 -22.5 -24 -75 -22.5 -24;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_0_2";
	setAttr ".t" -type "double3" 0 22.5 51 ;
createNode mesh -n "block_0_Shape2" -p "block_0_2";
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
	setAttr ".vt[0:7]"  -75 -22.5 24 75 -22.5 24 75 22.5 24 -75 22.5 24 75 22.5 -24 -75 22.5 -24 75 -22.5 -24 -75 -22.5 -24;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_1_0";
	setAttr ".t" -type "double3" -51 67.5 0 ;
createNode mesh -n "block_1_Shape0" -p "block_1_0";
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
	setAttr ".vt[0:7]"  -24 -22.5 75 24 -22.5 75 24 22.5 75 -24 22.5 75 24 22.5 -75 -24 22.5 -75 24 -22.5 -75 -24 -22.5 -75;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_1_1";
	setAttr ".t" -type "double3" 0 67.5 0 ;
createNode mesh -n "block_1_Shape1" -p "block_1_1";
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
	setAttr ".vt[0:7]"  -24 -22.5 75 24 -22.5 75 24 22.5 75 -24 22.5 75 24 22.5 -75 -24 22.5 -75 24 -22.5 -75 -24 -22.5 -75;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_1_2";
	setAttr ".t" -type "double3" 51 67.5 0 ;
createNode mesh -n "block_1_Shape2" -p "block_1_2";
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
	setAttr ".vt[0:7]"  -24 -22.5 75 24 -22.5 75 24 22.5 75 -24 22.5 75 24 22.5 -75 -24 22.5 -75 24 -22.5 -75 -24 -22.5 -75;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_2_0";
	setAttr ".t" -type "double3" 0 112.5 -51 ;
createNode mesh -n "block_2_Shape0" -p "block_2_0";
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
	setAttr ".vt[0:7]"  -75 -22.5 24 75 -22.5 24 75 22.5 24 -75 22.5 24 75 22.5 -24 -75 22.5 -24 75 -22.5 -24 -75 -22.5 -24;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_2_1";
	setAttr ".t" -type "double3" 0 112.5 0 ;
createNode mesh -n "block_2_Shape1" -p "block_2_1";
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
	setAttr ".vt[0:7]"  -75 -22.5 24 75 -22.5 24 75 22.5 24 -75 22.5 24 75 22.5 -24 -75 22.5 -24 75 -22.5 -24 -75 -22.5 -24;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_2_2";
	setAttr ".t" -type "double3" 0 112.5 51 ;
createNode mesh -n "block_2_Shape2" -p "block_2_2";
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
	setAttr ".vt[0:7]"  -75 -22.5 24 75 -22.5 24 75 22.5 24 -75 22.5 24 75 22.5 -24 -75 22.5 -24 75 -22.5 -24 -75 -22.5 -24;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_3_0";
	setAttr ".t" -type "double3" -51 157.5 0 ;
createNode mesh -n "block_3_Shape0" -p "block_3_0";
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
	setAttr ".vt[0:7]"  -24 -22.5 75 24 -22.5 75 24 22.5 75 -24 22.5 75 24 22.5 -75 -24 22.5 -75 24 -22.5 -75 -24 -22.5 -75;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_3_1";
	setAttr ".t" -type "double3" 0 157.5 0 ;
createNode mesh -n "block_3_Shape1" -p "block_3_1";
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
	setAttr ".vt[0:7]"  -24 -22.5 75 24 -22.5 75 24 22.5 75 -24 22.5 75 24 22.5 -75 -24 22.5 -75 24 -22.5 -75 -24 -22.5 -75;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_3_2";
	setAttr ".t" -type "double3" 51 157.5 0 ;
createNode mesh -n "block_3_Shape2" -p "block_3_2";
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
	setAttr ".vt[0:7]"  -24 -22.5 75 24 -22.5 75 24 22.5 75 -24 22.5 75 24 22.5 -75 -24 22.5 -75 24 -22.5 -75 -24 -22.5 -75;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_4_0";
	setAttr ".t" -type "double3" 0 202.5 -51 ;
createNode mesh -n "block_4_Shape0" -p "block_4_0";
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
	setAttr ".vt[0:7]"  -75 -22.5 24 75 -22.5 24 75 22.5 24 -75 22.5 24 75 22.5 -24 -75 22.5 -24 75 -22.5 -24 -75 -22.5 -24;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_4_1";
	setAttr ".t" -type "double3" 0 202.5 0 ;
createNode mesh -n "block_4_Shape1" -p "block_4_1";
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
	setAttr ".vt[0:7]"  -75 -22.5 24 75 -22.5 24 75 22.5 24 -75 22.5 24 75 22.5 -24 -75 22.5 -24 75 -22.5 -24 -75 -22.5 -24;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_4_2";
	setAttr ".t" -type "double3" 0 202.5 51 ;
createNode mesh -n "block_4_Shape2" -p "block_4_2";
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
	setAttr ".vt[0:7]"  -75 -22.5 24 75 -22.5 24 75 22.5 24 -75 22.5 24 75 22.5 -24 -75 22.5 -24 75 -22.5 -24 -75 -22.5 -24;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_5_0";
	setAttr ".t" -type "double3" -51 247.5 0 ;
createNode mesh -n "block_5_Shape0" -p "block_5_0";
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
	setAttr ".vt[0:7]"  -24 -22.5 75 24 -22.5 75 24 22.5 75 -24 22.5 75 24 22.5 -75 -24 22.5 -75 24 -22.5 -75 -24 -22.5 -75;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_5_1";
	setAttr ".t" -type "double3" 0 247.5 0 ;
createNode mesh -n "block_5_Shape1" -p "block_5_1";
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
	setAttr ".vt[0:7]"  -24 -22.5 75 24 -22.5 75 24 22.5 75 -24 22.5 75 24 22.5 -75 -24 22.5 -75 24 -22.5 -75 -24 -22.5 -75;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_5_2";
	setAttr ".t" -type "double3" 51 247.5 0 ;
createNode mesh -n "block_5_Shape2" -p "block_5_2";
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
	setAttr ".vt[0:7]"  -24 -22.5 75 24 -22.5 75 24 22.5 75 -24 22.5 75 24 22.5 -75 -24 22.5 -75 24 -22.5 -75 -24 -22.5 -75;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_6_0";
	setAttr ".t" -type "double3" 0 292.5 -51 ;
createNode mesh -n "block_6_Shape0" -p "block_6_0";
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
	setAttr ".vt[0:7]"  -75 -22.5 24 75 -22.5 24 75 22.5 24 -75 22.5 24 75 22.5 -24 -75 22.5 -24 75 -22.5 -24 -75 -22.5 -24;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_6_1";
	setAttr ".t" -type "double3" 0 292.5 0 ;
createNode mesh -n "block_6_Shape1" -p "block_6_1";
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
	setAttr ".vt[0:7]"  -75 -22.5 24 75 -22.5 24 75 22.5 24 -75 22.5 24 75 22.5 -24 -75 22.5 -24 75 -22.5 -24 -75 -22.5 -24;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_6_2";
	setAttr ".t" -type "double3" 0 292.5 51 ;
createNode mesh -n "block_6_Shape2" -p "block_6_2";
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
	setAttr ".vt[0:7]"  -75 -22.5 24 75 -22.5 24 75 22.5 24 -75 22.5 24 75 22.5 -24 -75 22.5 -24 75 -22.5 -24 -75 -22.5 -24;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_7_0";
	setAttr ".t" -type "double3" -51 337.5 0 ;
createNode mesh -n "block_7_Shape0" -p "block_7_0";
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
	setAttr ".vt[0:7]"  -24 -22.5 75 24 -22.5 75 24 22.5 75 -24 22.5 75 24 22.5 -75 -24 22.5 -75 24 -22.5 -75 -24 -22.5 -75;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_7_1";
	setAttr ".t" -type "double3" 0 337.5 0 ;
createNode mesh -n "block_7_Shape1" -p "block_7_1";
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
	setAttr ".vt[0:7]"  -24 -22.5 75 24 -22.5 75 24 22.5 75 -24 22.5 75 24 22.5 -75 -24 22.5 -75 24 -22.5 -75 -24 -22.5 -75;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "block_7_2";
	setAttr ".t" -type "double3" 51 337.5 0 ;
createNode mesh -n "block_7_Shape2" -p "block_7_2";
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
	setAttr ".vt[0:7]"  -24 -22.5 75 24 -22.5 75 24 22.5 75 -24 22.5 75 24 22.5 -75 -24 22.5 -75 24 -22.5 -75 -24 -22.5 -75;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "wreckingBall";
	setAttr ".t" -type "double3" -700 120 0 ;
createNode mesh -n "wreckingBallShape" -p "wreckingBall";
	setAttr -k off ".v";
	setAttr ".vir" yes;
	setAttr ".vif" yes;
	setAttr ".uvst[0].uvsn" -type "string" "map1";
	setAttr -s 439 ".uvst[0].uvsp";
	setAttr ".uvst[0].uvsp[0:249]" -type "float2" 0.025 0 0.05 0.05 0 0.05 0.075 0 0.1 0.05 0.125 0 0.15 0.05 0.17500001 0 0.2 0.05 0.225 0 0.25 0.05 0.275 0 0.3 0.05 0.325 0 0.35 0.05 0.375 0 0.4 0.05 0.425 0 0.45 0.05 0.475 0 0.5 0.05 0.525 0 0.55 0.05 0.57500005 0 0.6 0.05 0.625 0 0.65 0.05 0.67499995 0 0.7 0.05 0.725 0 0.75 0.05 0.775 0 0.8 0.05 0.82500005 0 0.85 0.05 0.875 0 0.9 0.05 0.92499995 0 0.95 0.05 0.975 0 1 0.05 0.05 0.1 0 0.1 0.1 0.1 0.15 0.1 0.2 0.1 0.25 0.1 0.3 0.1 0.35 0.1 0.4 0.1 0.45 0.1 0.5 0.1 0.55 0.1 0.6 0.1 0.65 0.1 0.7 0.1 0.75 0.1 0.8 0.1 0.85 0.1 0.9 0.1 0.95 0.1 1 0.1 0.05 0.15 0 0.15 0.1 0.15 0.15 0.15 0.2 0.15 0.25 0.15 0.3 0.15 0.35 0.15 0.4 0.15 0.45 0.15 0.5 0.15 0.55 0.15 0.6 0.15 0.65 0.15 0.7 0.15 0.75 0.15 0.8 0.15 0.85 0.15 0.9 0.15 0.95 0.15 1 0.15 0.05 0.2 0 0.2 0.1 0.2 0.15 0.2 0.2 0.2 0.25 0.2 0.3 0.2 0.35 0.2 0.4 0.2 0.45 0.2 0.5 0.2 0.55 0.2 0.6 0.2 0.65 0.2 0.7 0.2 0.75 0.2 0.8 0.2 0.85 0.2 0.9 0.2 0.95 0.2 1 0.2 0.05 0.25 0 0.25 0.1 0.25 0.15 0.25 0.2 0.25 0.25 0.25 0.3 0.25 0.35 0.25 0.4 0.25 0.45 0.25 0.5 0.25 0.55 0.25 0.6 0.25 0.65 0.25 0.7 0.25 0.75 0.25 0.8 0.25 0.85 0.25 0.9 0.25 0.95 0.25 1 0.25 0.05 0.3 0 0.3 0.1 0.3 0.15 0.3 0.2 0.3 0.25 0.3 0.3 0.3 0.35 0.3 0.4 0.3 0.45 0.3 0.5 0.3 0.55 0.3 0.6 0.3 0.65 0.3 0.7 0.3 0.75 0.3 0.8 0.3 0.85 0.3 0.9 0.3 0.95 0.3 1 0.3 0.05 0.35 0 0.35 0.1 0.35 0.15 0.35 0.2 0.35 0.25 0.35 0.3 0.35 0.35 0.35 0.4 0.35 0.45 0.35 0.5 0.35 0.55 0.35 0.6 0.35 0.65 0.35 0.7 0.35 0.75 0.35 0.8 0.35 0.85 0.35 0.9 0.35 0.95 0.35 1 0.35 0.05 0.4 0 0.4 0.1 0.4 0.15 0.4 0.2 0.4 0.25 0.4 0.3 0.4 0.35 0.4 0.4 0.4 0.45 0.4 0.5 0.4 0.55 0.4 0.6 0.4 0.65 0.4 0.7 0.4 0.75 0.4 0.8 0.4 0.85 0.4 0.9 0.4 0.95 0.4 1 0.4 0.05 0.45 0 0.45 0.1 0.45 0.15 0.45 0.2 0.45 0.25 0.45 0.3 0.45 0.35 0.45 0.4 0.45 0.45 0.45 0.5 0.45 0.55 0.45 0.6 0.45 0.65 0.45 0.7 0.45 0.75 0.45 0.8 0.45 0.85 0.45 0.9 0.45 0.95 0.45 1 0.45 0.05 0.5 0 0.5 0.1 0.5 0.15 0.5 0.2 0.5 0.25 0.5 0.3 0.5 0.35 0.5 0.4 0.5 0.45 0.5 0.5 0.5 0.55 0.5 0.6 0.5 0.65 0.5 0.7 0.5 0.75 0.5 0.8 0.5 0.85 0.5 0.9 0.5 0.95 0.5 1 0.5 0.05 0.55 0 0.55 0.1 0.55 0.15 0.55 0.2 0.55 0.25 0.55 0.3 0.55 0.35 0.55 0.4 0.55 0.45 0.55 0.5 0.55 0.55 0.55 0.6 0.55 0.65 0.55 0.7 0.55 0.75 0.55 0.8 0.55 0.85 0.55 0.9 0.55 0.95 0.55 ;
	setAttr ".uvst[0].uvsp[250:438]" -type "float2" 1 0.55 0.05 0.6 0 0.6 0.1 0.6 0.15 0.6 0.2 0.6 0.25 0.6 0.3 0.6 0.35 0.6 0.4 0.6 0.45 0.6 0.5 0.6 0.55 0.6 0.6 0.6 0.65 0.6 0.7 0.6 0.75 0.6 0.8 0.6 0.85 0.6 0.9 0.6 0.95 0.6 1 0.6 0.05 0.65 0 0.65 0.1 0.65 0.15 0.65 0.2 0.65 0.25 0.65 0.3 0.65 0.35 0.65 0.4 0.65 0.45 0.65 0.5 0.65 0.55 0.65 0.6 0.65 0.65 0.65 0.7 0.65 0.75 0.65 0.8 0.65 0.85 0.65 0.9 0.65 0.95 0.65 1 0.65 0.05 0.7 0 0.7 0.1 0.7 0.15 0.7 0.2 0.7 0.25 0.7 0.3 0.7 0.35 0.7 0.4 0.7 0.45 0.7 0.5 0.7 0.55 0.7 0.6 0.7 0.65 0.7 0.7 0.7 0.75 0.7 0.8 0.7 0.85 0.7 0.9 0.7 0.95 0.7 1 0.7 0.05 0.75 0 0.75 0.1 0.75 0.15 0.75 0.2 0.75 0.25 0.75 0.3 0.75 0.35 0.75 0.4 0.75 0.45 0.75 0.5 0.75 0.55 0.75 0.6 0.75 0.65 0.75 0.7 0.75 0.75 0.75 0.8 0.75 0.85 0.75 0.9 0.75 0.95 0.75 1 0.75 0.05 0.8 0 0.8 0.1 0.8 0.15 0.8 0.2 0.8 0.25 0.8 0.3 0.8 0.35 0.8 0.4 0.8 0.45 0.8 0.5 0.8 0.55 0.8 0.6 0.8 0.65 0.8 0.7 0.8 0.75 0.8 0.8 0.8 0.85 0.8 0.9 0.8 0.95 0.8 1 0.8 0.05 0.85 0 0.85 0.1 0.85 0.15 0.85 0.2 0.85 0.25 0.85 0.3 0.85 0.35 0.85 0.4 0.85 0.45 0.85 0.5 0.85 0.55 0.85 0.6 0.85 0.65 0.85 0.7 0.85 0.75 0.85 0.8 0.85 0.85 0.85 0.9 0.85 0.95 0.85 1 0.85 0.05 0.9 0 0.9 0.1 0.9 0.15 0.9 0.2 0.9 0.25 0.9 0.3 0.9 0.35 0.9 0.4 0.9 0.45 0.9 0.5 0.9 0.55 0.9 0.6 0.9 0.65 0.9 0.7 0.9 0.75 0.9 0.8 0.9 0.85 0.9 0.9 0.9 0.95 0.9 1 0.9 0.05 0.95 0 0.95 0.1 0.95 0.15 0.95 0.2 0.95 0.25 0.95 0.3 0.95 0.35 0.95 0.4 0.95 0.45 0.95 0.5 0.95 0.55 0.95 0.6 0.95 0.65 0.95 0.7 0.95 0.75 0.95 0.8 0.95 0.85 0.95 0.9 0.95 0.95 0.95 1 0.95 0.025 1 0.075 1 0.125 1 0.17500001 1 0.225 1 0.275 1 0.325 1 0.375 1 0.425 1 0.475 1 0.525 1 0.57500005 1 0.625 1 0.67499995 1 0.725 1 0.775 1 0.82500005 1 0.875 1 0.92499995 1 0.975 1 ;
	setAttr ".cuvs" -type "string" "map1";
	setAttr ".dcc" -type "string" "Ambient+Diffuse";
	setAttr ".covm[0]"  0 1 1;
	setAttr ".cdvm[0]"  0 1 1;
	setAttr -s 382 ".vt";
	setAttr ".vt[0:165]"  0 -50 0 7.821723252011544 -49.38441702975689 0 7.438900867482898 -49.38441702975689 -2.4170454101692465 6.327907036175018 -49.38441702975689 -4.597493575045509 4.597493575045509 -49.38441702975689 -6.327907036175018 2.417045410169247 -49.38441702975689 -7.438900867482898 0.0000000000000004789424172196181 -49.38441702975689 -7.821723252011544 -2.417045410169246 -49.38441702975689 -7.438900867482899 -4.597493575045508 -49.38441702975689 -6.327907036175018 -6.327907036175017 -49.38441702975689 -4.597493575045509 -7.438900867482898 -49.38441702975689 -2.4170454101692473 -7.821723252011544 -49.38441702975689 -0.0000000000000009578848344392363 -7.4389008674829 -49.38441702975689 2.4170454101692425 -6.327907036175019 -49.38441702975689 4.597493575045508 -4.597493575045509 -49.38441702975689 6.327907036175017 -2.417045410169248 -49.38441702975689 7.438900867482898 -0.0000000000000014368272516588544 -49.38441702975689 7.821723252011544 2.417045410169245 -49.38441702975689 7.438900867482899 4.597493575045507 -49.38441702975689 6.327907036175019 6.327907036175017 -49.38441702975689 4.59749357504551 7.438900867482898 -49.38441702975689 2.4170454101692482 15.450849718747369 -47.552825814757675 0 14.694631307311825 -47.552825814757675 -4.774575140626314 12.499999999999998 -47.552825814757675 -9.08178160006701 9.08178160006701 -47.552825814757675 -12.499999999999998 4.774575140626314 -47.552825814757675 -14.694631307311825 0.0000000000000009460916826085374 -47.552825814757675 -15.450849718747369 -4.774575140626313 -47.552825814757675 -14.694631307311827 -9.081781600067009 -47.552825814757675 -12.499999999999998 -12.499999999999996 -47.552825814757675 -9.081781600067012 -14.694631307311825 -47.552825814757675 -4.774575140626315 -15.450849718747369 -47.552825814757675 -0.000000000000001892183365217075 -14.694631307311829 -47.552825814757675 4.774575140626306 -12.5 -47.552825814757675 9.081781600067009 -9.081781600067012 -47.552825814757675 12.499999999999996 -4.7745751406263155 -47.552825814757675 14.694631307311825 -0.000000000000002838275047825612 -47.552825814757675 15.450849718747369 4.774575140626311 -47.552825814757675 14.694631307311827 9.081781600067007 -47.552825814757675 12.5 12.499999999999996 -47.552825814757675 9.081781600067014 14.694631307311825 -47.552825814757675 4.774575140626316 22.699524986977337 -44.550326209418394 0 21.588531155669457 -44.550326209418394 -7.014538985214754 18.364301478703425 -44.550326209418394 -13.342446021389772 13.342446021389772 -44.550326209418394 -18.364301478703425 7.014538985214755 -44.550326209418394 -21.588531155669457 0.0000000000000013899450308733581 -44.550326209418394 -22.699524986977337 -7.014538985214752 -44.550326209418394 -21.58853115566946 -13.34244602138977 -44.550326209418394 -18.364301478703425 -18.36430147870342 -44.550326209418394 -13.342446021389774 -21.588531155669457 -44.550326209418394 -7.014538985214756 -22.699524986977337 -44.550326209418394 -0.0000000000000027798900617467162 -21.58853115566946 -44.550326209418394 7.014538985214743 -18.364301478703425 -44.550326209418394 13.34244602138977 -13.342446021389774 -44.550326209418394 18.36430147870342 -7.014538985214758 -44.550326209418394 21.588531155669457 -0.000000000000004169835092620074 -44.550326209418394 22.699524986977337 7.01453898521475 -44.550326209418394 21.58853115566946 13.342446021389767 -44.550326209418394 18.364301478703425 18.36430147870342 -44.550326209418394 13.342446021389778 21.588531155669457 -44.550326209418394 7.014538985214759 29.389262614623657 -40.45084971874737 0 27.95084971874737 -40.45084971874737 -9.08178160006701 23.77641290737884 -40.45084971874737 -17.274575140626315 17.274575140626315 -40.45084971874737 -23.77641290737884 9.081781600067012 -40.45084971874737 -27.95084971874737 0.0000000000000017995733195149917 -40.45084971874737 -29.389262614623657 -9.081781600067009 -40.45084971874737 -27.950849718747374 -17.27457514062631 -40.45084971874737 -23.77641290737884 -23.776412907378838 -40.45084971874737 -17.274575140626318 -27.95084971874737 -40.45084971874737 -9.081781600067014 -29.389262614623657 -40.45084971874737 -0.0000000000000035991466390299834 -27.950849718747378 -40.45084971874737 9.081781600066996 -23.776412907378845 -40.45084971874737 17.27457514062631 -17.274575140626318 -40.45084971874737 23.776412907378838 -9.081781600067016 -40.45084971874737 27.95084971874737 -0.000000000000005398719958544975 -40.45084971874737 29.389262614623657 9.081781600067005 -40.45084971874737 27.950849718747374 17.274575140626308 -40.45084971874737 23.776412907378845 23.776412907378838 -40.45084971874737 17.27457514062632 27.95084971874737 -40.45084971874737 9.081781600067018 35.35533905932737 -35.35533905932738 0 33.62492559819786 -35.35533905932738 -10.925400611220525 28.60307014088421 -35.35533905932738 -20.78134688887267 20.78134688887267 -35.35533905932738 -28.60307014088421 10.925400611220526 -35.35533905932738 -33.62492559819786 0.0000000000000021648901405887327 -35.35533905932738 -35.35533905932737 -10.925400611220523 -35.35533905932738 -33.62492559819786 -20.781346888872665 -35.35533905932738 -28.60307014088421 -28.603070140884206 -35.35533905932738 -20.781346888872672 -33.62492559819786 -35.35533905932738 -10.925400611220528 -35.35533905932737 -35.35533905932738 -0.0000000000000043297802811774655 -33.62492559819787 -35.35533905932738 10.925400611220507 -28.603070140884213 -35.35533905932738 20.781346888872665 -20.781346888872672 -35.35533905932738 28.603070140884206 -10.92540061122053 -35.35533905932738 33.62492559819786 -0.000000000000006494670421766198 -35.35533905932738 35.35533905932737 10.925400611220518 -35.35533905932738 33.62492559819786 20.781346888872662 -35.35533905932738 28.603070140884213 28.603070140884206 -35.35533905932738 20.781346888872676 33.62492559819786 -35.35533905932738 10.925400611220532 40.45084971874737 -29.389262614623657 0 38.471044214690664 -29.389262614623657 -12.499999999999998 32.72542485937369 -29.389262614623657 -23.77641290737884 23.77641290737884 -29.389262614623657 -32.72542485937369 12.500000000000002 -29.389262614623657 -38.471044214690664 0.000000000000002476900181542729 -29.389262614623657 -40.45084971874737 -12.499999999999996 -29.389262614623657 -38.47104421469067 -23.776412907378834 -29.389262614623657 -32.72542485937369 -32.72542485937368 -29.389262614623657 -23.776412907378845 -38.471044214690664 -29.389262614623657 -12.500000000000004 -40.45084971874737 -29.389262614623657 -0.000000000000004953800363085458 -38.47104421469067 -29.389262614623657 12.499999999999979 -32.72542485937369 -29.389262614623657 23.776412907378834 -23.776412907378845 -29.389262614623657 32.72542485937368 -12.500000000000005 -29.389262614623657 38.471044214690664 -0.000000000000007430700544628187 -29.389262614623657 40.45084971874737 12.499999999999991 -29.389262614623657 38.47104421469067 23.77641290737883 -29.389262614623657 32.72542485937369 32.72542485937368 -29.389262614623657 23.776412907378848 38.471044214690664 -29.389262614623657 12.500000000000007 44.55032620941839 -22.69952498697734 0 42.369878044542126 -22.69952498697734 -13.766807903657913 36.041971008367106 -22.69952498697734 -26.186024730714966 26.186024730714966 -22.69952498697734 -36.041971008367106 13.766807903657915 -22.69952498697734 -42.369878044542126 0.0000000000000027279207196667333 -22.69952498697734 -44.55032620941839 -13.766807903657911 -22.69952498697734 -42.369878044542126 -26.18602473071496 -22.69952498697734 -36.041971008367106 -36.041971008367106 -22.69952498697734 -26.18602473071497 -42.369878044542126 -22.69952498697734 -13.766807903657918 -44.55032620941839 -22.69952498697734 -0.000000000000005455841439333467 -42.36987804454213 -22.69952498697734 13.766807903657892 -36.04197100836711 -22.69952498697734 26.18602473071496 -26.18602473071497 -22.69952498697734 36.041971008367106 -13.76680790365792 -22.69952498697734 42.369878044542126 -0.000000000000008183762159000198 -22.69952498697734 44.55032620941839 13.766807903657906 -22.69952498697734 42.369878044542126 26.186024730714955 -22.69952498697734 36.04197100836711 36.041971008367106 -22.69952498697734 26.186024730714976 42.369878044542126 -22.69952498697734 13.766807903657924 47.552825814757675 -15.450849718747373 0 45.22542485937368 -15.450849718747373 -14.694631307311825 38.471044214690664 -15.450849718747373 -27.95084971874737 27.95084971874737 -15.450849718747373 -38.471044214690664 14.694631307311829 -15.450849718747373 -45.22542485937368 0.0000000000000029117707962227307 -15.450849718747373 -47.552825814757675 -14.694631307311823 -15.450849718747373 -45.22542485937369 -27.950849718747364 -15.450849718747373 -38.471044214690664 -38.47104421469066 -15.450849718747373 -27.950849718747374 -45.22542485937368 -15.450849718747373 -14.69463130731183 -47.552825814757675 -15.450849718747373 -0.000000000000005823541592445461 -45.22542485937369 -15.450849718747373 14.694631307311802 -38.47104421469067 -15.450849718747373 27.950849718747364 -27.950849718747374 -15.450849718747373 38.47104421469066 -14.694631307311834 -15.450849718747373 45.22542485937368 -0.000000000000008735312388668192 -15.450849718747373 47.552825814757675 14.694631307311818 -15.450849718747373 45.22542485937369 27.95084971874736 -15.450849718747373 38.47104421469067 38.47104421469066 -15.450849718747373 27.95084971874738 45.22542485937368 -15.450849718747373 14.694631307311836 49.38441702975689 -7.821723252011546 0 46.96737161958764 -7.821723252011546 -15.260624119494441 39.95283263437289 -7.821723252011546 -29.02743202315236 29.02743202315236 -7.821723252011546 -39.95283263437289 15.260624119494445 -7.821723252011546 -46.96737161958764;
	setAttr ".vt[166:331]"  0.000000000000003023923412162491 -7.821723252011546 -49.38441702975689 -15.26062411949444 -7.821723252011546 -46.967371619587645 -29.027432023152354 -7.821723252011546 -39.95283263437289 -39.95283263437288 -7.821723252011546 -29.027432023152365 -46.96737161958764 -7.821723252011546 -15.260624119494446 -49.38441702975689 -7.821723252011546 -0.000000000000006047846824324982 -46.96737161958765 -7.821723252011546 15.260624119494416 -39.952832634372896 -7.821723252011546 29.027432023152354 -29.027432023152365 -7.821723252011546 39.95283263437288 -15.26062411949445 -7.821723252011546 46.96737161958764 -0.000000000000009071770236487472 -7.821723252011546 49.38441702975689 15.260624119494432 -7.821723252011546 46.967371619587645 29.02743202315235 -7.821723252011546 39.952832634372896 39.95283263437288 -7.821723252011546 29.02743202315237 46.96737161958764 -7.821723252011546 15.260624119494452 50 -0.000000000000003061616997868383 0 47.552825814757675 -0.000000000000003061616997868383 -15.450849718747369 40.45084971874737 -0.000000000000003061616997868383 -29.389262614623657 29.389262614623657 -0.000000000000003061616997868383 -40.45084971874737 15.450849718747373 -0.000000000000003061616997868383 -47.552825814757675 0.000000000000003061616997868383 -0.000000000000003061616997868383 -50 -15.450849718747367 -0.000000000000003061616997868383 -47.55282581475768 -29.38926261462365 -0.000000000000003061616997868383 -40.45084971874737 -40.450849718747364 -0.000000000000003061616997868383 -29.38926261462366 -47.552825814757675 -0.000000000000003061616997868383 -15.450849718747376 -50 -0.000000000000003061616997868383 -0.000000000000006123233995736766 -47.55282581475769 -0.000000000000003061616997868383 15.450849718747344 -40.45084971874738 -0.000000000000003061616997868383 29.38926261462365 -29.38926261462366 -0.000000000000003061616997868383 40.450849718747364 -15.450849718747378 -0.000000000000003061616997868383 47.552825814757675 -0.000000000000009184850993605149 -0.000000000000003061616997868383 50 15.450849718747362 -0.000000000000003061616997868383 47.55282581475768 29.389262614623647 -0.000000000000003061616997868383 40.45084971874738 40.450849718747364 -0.000000000000003061616997868383 29.389262614623668 47.552825814757675 -0.000000000000003061616997868383 15.450849718747381 49.38441702975689 7.821723252011529 0 46.96737161958764 7.821723252011529 -15.260624119494441 39.95283263437289 7.821723252011529 -29.02743202315236 29.02743202315236 7.821723252011529 -39.95283263437289 15.260624119494445 7.821723252011529 -46.96737161958764 0.000000000000003023923412162491 7.821723252011529 -49.38441702975689 -15.26062411949444 7.821723252011529 -46.967371619587645 -29.027432023152354 7.821723252011529 -39.95283263437289 -39.95283263437288 7.821723252011529 -29.027432023152365 -46.96737161958764 7.821723252011529 -15.260624119494446 -49.38441702975689 7.821723252011529 -0.000000000000006047846824324982 -46.96737161958765 7.821723252011529 15.260624119494416 -39.952832634372896 7.821723252011529 29.027432023152354 -29.027432023152365 7.821723252011529 39.95283263437288 -15.26062411949445 7.821723252011529 46.96737161958764 -0.000000000000009071770236487472 7.821723252011529 49.38441702975689 15.260624119494432 7.821723252011529 46.967371619587645 29.02743202315235 7.821723252011529 39.952832634372896 39.95283263437288 7.821723252011529 29.02743202315237 46.96737161958764 7.821723252011529 15.260624119494452 47.55282581475768 15.450849718747367 0 45.22542485937369 15.450849718747367 -14.694631307311829 38.47104421469067 15.450849718747367 -27.950849718747374 27.950849718747374 15.450849718747367 -38.47104421469067 14.69463130731183 15.450849718747367 -45.22542485937369 0.000000000000002911770796222731 15.450849718747367 -47.55282581475768 -14.694631307311825 15.450849718747367 -45.22542485937369 -27.950849718747367 15.450849718747367 -38.47104421469067 -38.471044214690664 15.450849718747367 -27.950849718747378 -45.22542485937369 15.450849718747367 -14.694631307311834 -47.55282581475768 15.450849718747367 -0.000000000000005823541592445462 -45.225424859373696 15.450849718747367 14.694631307311804 -38.47104421469068 15.450849718747367 27.950849718747367 -27.950849718747378 15.450849718747367 38.471044214690664 -14.694631307311836 15.450849718747367 45.22542485937369 -0.000000000000008735312388668194 15.450849718747367 47.55282581475768 14.69463130731182 15.450849718747367 45.22542485937369 27.950849718747364 15.450849718747367 38.47104421469068 38.471044214690664 15.450849718747367 27.950849718747385 45.22542485937369 15.450849718747367 14.69463130731184 44.550326209418394 22.699524986977334 0 42.36987804454213 22.699524986977334 -13.766807903657915 36.04197100836711 22.699524986977334 -26.18602473071497 26.18602473071497 22.699524986977334 -36.04197100836711 13.766807903657918 22.699524986977334 -42.36987804454213 0.0000000000000027279207196667337 22.699524986977334 -44.550326209418394 -13.766807903657913 22.699524986977334 -42.36987804454213 -26.186024730714966 22.699524986977334 -36.04197100836711 -36.041971008367106 22.699524986977334 -26.186024730714973 -42.36987804454213 22.699524986977334 -13.76680790365792 -44.550326209418394 22.699524986977334 -0.0000000000000054558414393334674 -42.36987804454214 22.699524986977334 13.766807903657893 -36.04197100836712 22.699524986977334 26.186024730714966 -26.186024730714973 22.699524986977334 36.041971008367106 -13.766807903657924 22.699524986977334 42.36987804454213 -0.0000000000000081837621590002 22.699524986977334 44.550326209418394 13.766807903657908 22.699524986977334 42.36987804454213 26.18602473071496 22.699524986977334 36.04197100836712 36.041971008367106 22.699524986977334 26.18602473071498 42.36987804454213 22.699524986977334 13.766807903657925 40.45084971874737 29.38926261462365 0 38.471044214690664 29.38926261462365 -12.499999999999998 32.72542485937369 29.38926261462365 -23.77641290737884 23.77641290737884 29.38926261462365 -32.72542485937369 12.500000000000002 29.38926261462365 -38.471044214690664 0.000000000000002476900181542729 29.38926261462365 -40.45084971874737 -12.499999999999996 29.38926261462365 -38.47104421469067 -23.776412907378834 29.38926261462365 -32.72542485937369 -32.72542485937368 29.38926261462365 -23.776412907378845 -38.471044214690664 29.38926261462365 -12.500000000000004 -40.45084971874737 29.38926261462365 -0.000000000000004953800363085458 -38.47104421469067 29.38926261462365 12.499999999999979 -32.72542485937369 29.38926261462365 23.776412907378834 -23.776412907378845 29.38926261462365 32.72542485937368 -12.500000000000005 29.38926261462365 38.471044214690664 -0.000000000000007430700544628187 29.38926261462365 40.45084971874737 12.499999999999991 29.38926261462365 38.47104421469067 23.77641290737883 29.38926261462365 32.72542485937369 32.72542485937368 29.38926261462365 23.776412907378848 38.471044214690664 29.38926261462365 12.500000000000007 35.35533905932738 35.35533905932737 0 33.62492559819787 35.35533905932737 -10.925400611220526 28.603070140884217 35.35533905932737 -20.781346888872672 20.781346888872672 35.35533905932737 -28.603070140884217 10.925400611220528 35.35533905932737 -33.62492559819787 0.000000000000002164890140588733 35.35533905932737 -35.35533905932738 -10.925400611220525 35.35533905932737 -33.62492559819787 -20.78134688887267 35.35533905932737 -28.603070140884217 -28.603070140884213 35.35533905932737 -20.781346888872676 -33.62492559819787 35.35533905932737 -10.92540061122053 -35.35533905932738 35.35533905932737 -0.000000000000004329780281177466 -33.624925598197876 35.35533905932737 10.925400611220509 -28.60307014088422 35.35533905932737 20.78134688887267 -20.781346888872676 35.35533905932737 28.603070140884213 -10.925400611220532 35.35533905932737 33.62492559819787 -0.0000000000000064946704217662 35.35533905932737 35.35533905932738 10.925400611220521 35.35533905932737 33.62492559819787 20.781346888872665 35.35533905932737 28.60307014088422 28.603070140884213 35.35533905932737 20.78134688887268 33.62492559819787 35.35533905932737 10.925400611220534 29.38926261462366 40.450849718747364 0 27.950849718747374 40.450849718747364 -9.081781600067012 23.776412907378845 40.450849718747364 -17.274575140626318 17.274575140626318 40.450849718747364 -23.776412907378845 9.081781600067012 40.450849718747364 -27.950849718747374 0.000000000000001799573319514992 40.450849718747364 -29.38926261462366 -9.08178160006701 40.450849718747364 -27.950849718747378 -17.274575140626315 40.450849718747364 -23.776412907378845 -23.77641290737884 40.450849718747364 -17.27457514062632 -27.950849718747374 40.450849718747364 -9.081781600067014 -29.38926261462366 40.450849718747364 -0.000000000000003599146639029984 -27.95084971874738 40.450849718747364 9.081781600066996 -23.776412907378848 40.450849718747364 17.274575140626315 -17.27457514062632 40.450849718747364 23.77641290737884 -9.081781600067016 40.450849718747364 27.950849718747374 -0.0000000000000053987199585449755 40.450849718747364 29.38926261462366 9.081781600067007 40.450849718747364 27.950849718747378 17.27457514062631 40.450849718747364 23.776412907378848 23.77641290737884 40.450849718747364 17.274575140626325 27.950849718747374 40.450849718747364 9.081781600067018 22.699524986977345 44.55032620941839 0 21.588531155669465 44.55032620941839 -7.014538985214756 18.364301478703428 44.55032620941839 -13.342446021389776 13.342446021389776 44.55032620941839 -18.364301478703428 7.014538985214757 44.55032620941839 -21.588531155669465 0.0000000000000013899450308733585 44.55032620941839 -22.699524986977345 -7.014538985214755 44.55032620941839 -21.588531155669465 -13.342446021389774 44.55032620941839 -18.364301478703428 -18.364301478703428 44.55032620941839 -13.34244602138978 -21.588531155669465 44.55032620941839 -7.014538985214759 -22.699524986977345 44.55032620941839 -0.000000000000002779890061746717;
	setAttr ".vt[332:381]"  -21.588531155669468 44.55032620941839 7.014538985214744 -18.36430147870343 44.55032620941839 13.342446021389774 -13.34244602138978 44.55032620941839 18.364301478703428 -7.0145389852147595 44.55032620941839 21.588531155669465 -0.000000000000004169835092620075 44.55032620941839 22.699524986977345 7.014538985214752 44.55032620941839 21.588531155669465 13.34244602138977 44.55032620941839 18.36430147870343 18.364301478703428 44.55032620941839 13.342446021389781 21.588531155669465 44.55032620941839 7.014538985214761 15.450849718747376 47.552825814757675 0 14.694631307311832 47.552825814757675 -4.7745751406263155 12.500000000000004 47.552825814757675 -9.081781600067014 9.081781600067014 47.552825814757675 -12.500000000000004 4.774575140626316 47.552825814757675 -14.694631307311832 0.0000000000000009460916826085378 47.552825814757675 -15.450849718747376 -4.774575140626315 47.552825814757675 -14.694631307311834 -9.081781600067012 47.552825814757675 -12.500000000000004 -12.500000000000004 47.552825814757675 -9.081781600067016 -14.694631307311832 47.552825814757675 -4.774575140626317 -15.450849718747376 47.552825814757675 -0.0000000000000018921833652170757 -14.694631307311836 47.552825814757675 4.7745751406263075 -12.500000000000005 47.552825814757675 9.081781600067012 -9.081781600067016 47.552825814757675 12.500000000000004 -4.774575140626318 47.552825814757675 14.694631307311832 -0.000000000000002838275047825613 47.552825814757675 15.450849718747376 4.774575140626313 47.552825814757675 14.694631307311834 9.08178160006701 47.552825814757675 12.500000000000005 12.500000000000004 47.552825814757675 9.081781600067018 14.694631307311832 47.552825814757675 4.774575140626319 7.821723252011549 49.38441702975688 0 7.438900867482903 49.38441702975688 -2.4170454101692482 6.327907036175023 49.38441702975688 -4.597493575045512 4.597493575045512 49.38441702975688 -6.327907036175023 2.4170454101692487 49.38441702975688 -7.438900867482903 0.0000000000000004789424172196185 49.38441702975688 -7.821723252011549 -2.417045410169248 49.38441702975688 -7.438900867482904 -4.597493575045511 49.38441702975688 -6.327907036175023 -6.327907036175022 49.38441702975688 -4.597493575045513 -7.438900867482903 49.38441702975688 -2.417045410169249 -7.821723252011549 49.38441702975688 -0.000000000000000957884834439237 -7.4389008674829045 49.38441702975688 2.4170454101692442 -6.327907036175024 49.38441702975688 4.597493575045511 -4.597493575045513 49.38441702975688 6.327907036175022 -2.4170454101692496 49.38441702975688 7.438900867482903 -0.0000000000000014368272516588554 49.38441702975688 7.821723252011549 2.417045410169247 49.38441702975688 7.438900867482904 4.59749357504551 49.38441702975688 6.327907036175024 6.327907036175022 49.38441702975688 4.5974935750455135 7.438900867482903 49.38441702975688 2.41704541016925 0 50 0;
	setAttr -s 780 ".ed";
	setAttr ".ed[0:165]"  0 2 1 2 1 1 1 0 1 0 3 1 3 2 1 0 4 1 4 3 1 0 5 1 5 4 1 0 6 1 6 5 1 0 7 1 7 6 1 0 8 1 8 7 1 0 9 1 9 8 1 0 10 1 10 9 1 0 11 1 11 10 1 0 12 1 12 11 1 0 13 1 13 12 1 0 14 1 14 13 1 0 15 1 15 14 1 0 16 1 16 15 1 0 17 1 17 16 1 0 18 1 18 17 1 0 19 1 19 18 1 0 20 1 20 19 1 1 20 1 2 22 1 22 21 1 21 1 1 3 23 1 23 22 1 4 24 1 24 23 1 5 25 1 25 24 1 6 26 1 26 25 1 7 27 1 27 26 1 8 28 1 28 27 1 9 29 1 29 28 1 10 30 1 30 29 1 11 31 1 31 30 1 12 32 1 32 31 1 13 33 1 33 32 1 14 34 1 34 33 1 15 35 1 35 34 1 16 36 1 36 35 1 17 37 1 37 36 1 18 38 1 38 37 1 19 39 1 39 38 1 20 40 1 40 39 1 21 40 1 22 42 1 42 41 1 41 21 1 23 43 1 43 42 1 24 44 1 44 43 1 25 45 1 45 44 1 26 46 1 46 45 1 27 47 1 47 46 1 28 48 1 48 47 1 29 49 1 49 48 1 30 50 1 50 49 1 31 51 1 51 50 1 32 52 1 52 51 1 33 53 1 53 52 1 34 54 1 54 53 1 35 55 1 55 54 1 36 56 1 56 55 1 37 57 1 57 56 1 38 58 1 58 57 1 39 59 1 59 58 1 40 60 1 60 59 1 41 60 1 42 62 1 62 61 1 61 41 1 43 63 1 63 62 1 44 64 1 64 63 1 45 65 1 65 64 1 46 66 1 66 65 1 47 67 1 67 66 1 48 68 1 68 67 1 49 69 1 69 68 1 50 70 1 70 69 1 51 71 1 71 70 1 52 72 1 72 71 1 53 73 1 73 72 1 54 74 1 74 73 1 55 75 1 75 74 1 56 76 1 76 75 1 57 77 1 77 76 1 58 78 1 78 77 1 59 79 1 79 78 1 60 80 1 80 79 1 61 80 1 62 82 1 82 81 1 81 61 1 63 83 1 83 82 1 64 84 1;
	setAttr ".ed[166:331]"  84 83 1 65 85 1 85 84 1 66 86 1 86 85 1 67 87 1 87 86 1 68 88 1 88 87 1 69 89 1 89 88 1 70 90 1 90 89 1 71 91 1 91 90 1 72 92 1 92 91 1 73 93 1 93 92 1 74 94 1 94 93 1 75 95 1 95 94 1 76 96 1 96 95 1 77 97 1 97 96 1 78 98 1 98 97 1 79 99 1 99 98 1 80 100 1 100 99 1 81 100 1 82 102 1 102 101 1 101 81 1 83 103 1 103 102 1 84 104 1 104 103 1 85 105 1 105 104 1 86 106 1 106 105 1 87 107 1 107 106 1 88 108 1 108 107 1 89 109 1 109 108 1 90 110 1 110 109 1 91 111 1 111 110 1 92 112 1 112 111 1 93 113 1 113 112 1 94 114 1 114 113 1 95 115 1 115 114 1 96 116 1 116 115 1 97 117 1 117 116 1 98 118 1 118 117 1 99 119 1 119 118 1 100 120 1 120 119 1 101 120 1 102 122 1 122 121 1 121 101 1 103 123 1 123 122 1 104 124 1 124 123 1 105 125 1 125 124 1 106 126 1 126 125 1 107 127 1 127 126 1 108 128 1 128 127 1 109 129 1 129 128 1 110 130 1 130 129 1 111 131 1 131 130 1 112 132 1 132 131 1 113 133 1 133 132 1 114 134 1 134 133 1 115 135 1 135 134 1 116 136 1 136 135 1 117 137 1 137 136 1 118 138 1 138 137 1 119 139 1 139 138 1 120 140 1 140 139 1 121 140 1 122 142 1 142 141 1 141 121 1 123 143 1 143 142 1 124 144 1 144 143 1 125 145 1 145 144 1 126 146 1 146 145 1 127 147 1 147 146 1 128 148 1 148 147 1 129 149 1 149 148 1 130 150 1 150 149 1 131 151 1 151 150 1 132 152 1 152 151 1 133 153 1 153 152 1 134 154 1 154 153 1 135 155 1 155 154 1 136 156 1 156 155 1 137 157 1 157 156 1 138 158 1 158 157 1 139 159 1 159 158 1 140 160 1 160 159 1 141 160 1 142 162 1 162 161 1 161 141 1 143 163 1 163 162 1 144 164 1 164 163 1 145 165 1 165 164 1 146 166 1 166 165 1 147 167 1;
	setAttr ".ed[332:497]"  167 166 1 148 168 1 168 167 1 149 169 1 169 168 1 150 170 1 170 169 1 151 171 1 171 170 1 152 172 1 172 171 1 153 173 1 173 172 1 154 174 1 174 173 1 155 175 1 175 174 1 156 176 1 176 175 1 157 177 1 177 176 1 158 178 1 178 177 1 159 179 1 179 178 1 160 180 1 180 179 1 161 180 1 162 182 1 182 181 1 181 161 1 163 183 1 183 182 1 164 184 1 184 183 1 165 185 1 185 184 1 166 186 1 186 185 1 167 187 1 187 186 1 168 188 1 188 187 1 169 189 1 189 188 1 170 190 1 190 189 1 171 191 1 191 190 1 172 192 1 192 191 1 173 193 1 193 192 1 174 194 1 194 193 1 175 195 1 195 194 1 176 196 1 196 195 1 177 197 1 197 196 1 178 198 1 198 197 1 179 199 1 199 198 1 180 200 1 200 199 1 181 200 1 182 202 1 202 201 1 201 181 1 183 203 1 203 202 1 184 204 1 204 203 1 185 205 1 205 204 1 186 206 1 206 205 1 187 207 1 207 206 1 188 208 1 208 207 1 189 209 1 209 208 1 190 210 1 210 209 1 191 211 1 211 210 1 192 212 1 212 211 1 193 213 1 213 212 1 194 214 1 214 213 1 195 215 1 215 214 1 196 216 1 216 215 1 197 217 1 217 216 1 198 218 1 218 217 1 199 219 1 219 218 1 200 220 1 220 219 1 201 220 1 202 222 1 222 221 1 221 201 1 203 223 1 223 222 1 204 224 1 224 223 1 205 225 1 225 224 1 206 226 1 226 225 1 207 227 1 227 226 1 208 228 1 228 227 1 209 229 1 229 228 1 210 230 1 230 229 1 211 231 1 231 230 1 212 232 1 232 231 1 213 233 1 233 232 1 214 234 1 234 233 1 215 235 1 235 234 1 216 236 1 236 235 1 217 237 1 237 236 1 218 238 1 238 237 1 219 239 1 239 238 1 220 240 1 240 239 1 221 240 1 222 242 1 242 241 1 241 221 1 223 243 1 243 242 1 224 244 1 244 243 1 225 245 1 245 244 1 226 246 1 246 245 1 227 247 1 247 246 1 228 248 1 248 247 1 229 249 1 249 248 1 230 250 1;
	setAttr ".ed[498:663]"  250 249 1 231 251 1 251 250 1 232 252 1 252 251 1 233 253 1 253 252 1 234 254 1 254 253 1 235 255 1 255 254 1 236 256 1 256 255 1 237 257 1 257 256 1 238 258 1 258 257 1 239 259 1 259 258 1 240 260 1 260 259 1 241 260 1 242 262 1 262 261 1 261 241 1 243 263 1 263 262 1 244 264 1 264 263 1 245 265 1 265 264 1 246 266 1 266 265 1 247 267 1 267 266 1 248 268 1 268 267 1 249 269 1 269 268 1 250 270 1 270 269 1 251 271 1 271 270 1 252 272 1 272 271 1 253 273 1 273 272 1 254 274 1 274 273 1 255 275 1 275 274 1 256 276 1 276 275 1 257 277 1 277 276 1 258 278 1 278 277 1 259 279 1 279 278 1 260 280 1 280 279 1 261 280 1 262 282 1 282 281 1 281 261 1 263 283 1 283 282 1 264 284 1 284 283 1 265 285 1 285 284 1 266 286 1 286 285 1 267 287 1 287 286 1 268 288 1 288 287 1 269 289 1 289 288 1 270 290 1 290 289 1 271 291 1 291 290 1 272 292 1 292 291 1 273 293 1 293 292 1 274 294 1 294 293 1 275 295 1 295 294 1 276 296 1 296 295 1 277 297 1 297 296 1 278 298 1 298 297 1 279 299 1 299 298 1 280 300 1 300 299 1 281 300 1 282 302 1 302 301 1 301 281 1 283 303 1 303 302 1 284 304 1 304 303 1 285 305 1 305 304 1 286 306 1 306 305 1 287 307 1 307 306 1 288 308 1 308 307 1 289 309 1 309 308 1 290 310 1 310 309 1 291 311 1 311 310 1 292 312 1 312 311 1 293 313 1 313 312 1 294 314 1 314 313 1 295 315 1 315 314 1 296 316 1 316 315 1 297 317 1 317 316 1 298 318 1 318 317 1 299 319 1 319 318 1 300 320 1 320 319 1 301 320 1 302 322 1 322 321 1 321 301 1 303 323 1 323 322 1 304 324 1 324 323 1 305 325 1 325 324 1 306 326 1 326 325 1 307 327 1 327 326 1 308 328 1 328 327 1 309 329 1 329 328 1 310 330 1 330 329 1 311 331 1 331 330 1 312 332 1 332 331 1 313 333 1;
	setAttr ".ed[664:779]"  333 332 1 314 334 1 334 333 1 315 335 1 335 334 1 316 336 1 336 335 1 317 337 1 337 336 1 318 338 1 338 337 1 319 339 1 339 338 1 320 340 1 340 339 1 321 340 1 322 342 1 342 341 1 341 321 1 323 343 1 343 342 1 324 344 1 344 343 1 325 345 1 345 344 1 326 346 1 346 345 1 327 347 1 347 346 1 328 348 1 348 347 1 329 349 1 349 348 1 330 350 1 350 349 1 331 351 1 351 350 1 332 352 1 352 351 1 333 353 1 353 352 1 334 354 1 354 353 1 335 355 1 355 354 1 336 356 1 356 355 1 337 357 1 357 356 1 338 358 1 358 357 1 339 359 1 359 358 1 340 360 1 360 359 1 341 360 1 342 362 1 362 361 1 361 341 1 343 363 1 363 362 1 344 364 1 364 363 1 345 365 1 365 364 1 346 366 1 366 365 1 347 367 1 367 366 1 348 368 1 368 367 1 349 369 1 369 368 1 350 370 1 370 369 1 351 371 1 371 370 1 352 372 1 372 371 1 353 373 1 373 372 1 354 374 1 374 373 1 355 375 1 375 374 1 356 376 1 376 375 1 357 377 1 377 376 1 358 378 1 378 377 1 359 379 1 379 378 1 360 380 1 380 379 1 361 380 1 362 381 1 381 361 1 363 381 1 364 381 1 365 381 1 366 381 1 367 381 1 368 381 1 369 381 1 370 381 1 371 381 1 372 381 1 373 381 1 374 381 1 375 381 1 376 381 1 377 381 1 378 381 1 379 381 1 380 381 1;
	setAttr -s 400 -ch 1560 ".fc[0:249]" -type "polyFaces" 
		f 3 0 1 2 mu 0 3 0 1 2 
		f 3 3 4 -1 mu 0 3 3 4 1 
		f 3 5 6 -4 mu 0 3 5 6 4 
		f 3 7 8 -6 mu 0 3 7 8 6 
		f 3 9 10 -8 mu 0 3 9 10 8 
		f 3 11 12 -10 mu 0 3 11 12 10 
		f 3 13 14 -12 mu 0 3 13 14 12 
		f 3 15 16 -14 mu 0 3 15 16 14 
		f 3 17 18 -16 mu 0 3 17 18 16 
		f 3 19 20 -18 mu 0 3 19 20 18 
		f 3 21 22 -20 mu 0 3 21 22 20 
		f 3 23 24 -22 mu 0 3 23 24 22 
		f 3 25 26 -24 mu 0 3 25 26 24 
		f 3 27 28 -26 mu 0 3 27 28 26 
		f 3 29 30 -28 mu 0 3 29 30 28 
		f 3 31 32 -30 mu 0 3 31 32 30 
		f 3 33 34 -32 mu 0 3 33 34 32 
		f 3 35 36 -34 mu 0 3 35 36 34 
		f 3 37 38 -36 mu 0 3 37 38 36 
		f 3 -3 39 -38 mu 0 3 39 40 38 
		f 4 -2 40 41 42 mu 0 4 2 1 41 42 
		f 4 -5 43 44 -41 mu 0 4 1 4 43 41 
		f 4 -7 45 46 -44 mu 0 4 4 6 44 43 
		f 4 -9 47 48 -46 mu 0 4 6 8 45 44 
		f 4 -11 49 50 -48 mu 0 4 8 10 46 45 
		f 4 -13 51 52 -50 mu 0 4 10 12 47 46 
		f 4 -15 53 54 -52 mu 0 4 12 14 48 47 
		f 4 -17 55 56 -54 mu 0 4 14 16 49 48 
		f 4 -19 57 58 -56 mu 0 4 16 18 50 49 
		f 4 -21 59 60 -58 mu 0 4 18 20 51 50 
		f 4 -23 61 62 -60 mu 0 4 20 22 52 51 
		f 4 -25 63 64 -62 mu 0 4 22 24 53 52 
		f 4 -27 65 66 -64 mu 0 4 24 26 54 53 
		f 4 -29 67 68 -66 mu 0 4 26 28 55 54 
		f 4 -31 69 70 -68 mu 0 4 28 30 56 55 
		f 4 -33 71 72 -70 mu 0 4 30 32 57 56 
		f 4 -35 73 74 -72 mu 0 4 32 34 58 57 
		f 4 -37 75 76 -74 mu 0 4 34 36 59 58 
		f 4 -39 77 78 -76 mu 0 4 36 38 60 59 
		f 4 -40 -43 79 -78 mu 0 4 38 40 61 60 
		f 4 -42 80 81 82 mu 0 4 42 41 62 63 
		f 4 -45 83 84 -81 mu 0 4 41 43 64 62 
		f 4 -47 85 86 -84 mu 0 4 43 44 65 64 
		f 4 -49 87 88 -86 mu 0 4 44 45 66 65 
		f 4 -51 89 90 -88 mu 0 4 45 46 67 66 
		f 4 -53 91 92 -90 mu 0 4 46 47 68 67 
		f 4 -55 93 94 -92 mu 0 4 47 48 69 68 
		f 4 -57 95 96 -94 mu 0 4 48 49 70 69 
		f 4 -59 97 98 -96 mu 0 4 49 50 71 70 
		f 4 -61 99 100 -98 mu 0 4 50 51 72 71 
		f 4 -63 101 102 -100 mu 0 4 51 52 73 72 
		f 4 -65 103 104 -102 mu 0 4 52 53 74 73 
		f 4 -67 105 106 -104 mu 0 4 53 54 75 74 
		f 4 -69 107 108 -106 mu 0 4 54 55 76 75 
		f 4 -71 109 110 -108 mu 0 4 55 56 77 76 
		f 4 -73 111 112 -110 mu 0 4 56 57 78 77 
		f 4 -75 113 114 -112 mu 0 4 57 58 79 78 
		f 4 -77 115 116 -114 mu 0 4 58 59 80 79 
		f 4 -79 117 118 -116 mu 0 4 59 60 81 80 
		f 4 -80 -83 119 -118 mu 0 4 60 61 82 81 
		f 4 -82 120 121 122 mu 0 4 63 62 83 84 
		f 4 -85 123 124 -121 mu 0 4 62 64 85 83 
		f 4 -87 125 126 -124 mu 0 4 64 65 86 85 
		f 4 -89 127 128 -126 mu 0 4 65 66 87 86 
		f 4 -91 129 130 -128 mu 0 4 66 67 88 87 
		f 4 -93 131 132 -130 mu 0 4 67 68 89 88 
		f 4 -95 133 134 -132 mu 0 4 68 69 90 89 
		f 4 -97 135 136 -134 mu 0 4 69 70 91 90 
		f 4 -99 137 138 -136 mu 0 4 70 71 92 91 
		f 4 -101 139 140 -138 mu 0 4 71 72 93 92 
		f 4 -103 141 142 -140 mu 0 4 72 73 94 93 
		f 4 -105 143 144 -142 mu 0 4 73 74 95 94 
		f 4 -107 145 146 -144 mu 0 4 74 75 96 95 
		f 4 -109 147 148 -146 mu 0 4 75 76 97 96 
		f 4 -111 149 150 -148 mu 0 4 76 77 98 97 
		f 4 -113 151 152 -150 mu 0 4 77 78 99 98 
		f 4 -115 153 154 -152 mu 0 4 78 79 100 99 
		f 4 -117 155 156 -154 mu 0 4 79 80 101 100 
		f 4 -119 157 158 -156 mu 0 4 80 81 102 101 
		f 4 -120 -123 159 -158 mu 0 4 81 82 103 102 
		f 4 -122 160 161 162 mu 0 4 84 83 104 105 
		f 4 -125 163 164 -161 mu 0 4 83 85 106 104 
		f 4 -127 165 166 -164 mu 0 4 85 86 107 106 
		f 4 -129 167 168 -166 mu 0 4 86 87 108 107 
		f 4 -131 169 170 -168 mu 0 4 87 88 109 108 
		f 4 -133 171 172 -170 mu 0 4 88 89 110 109 
		f 4 -135 173 174 -172 mu 0 4 89 90 111 110 
		f 4 -137 175 176 -174 mu 0 4 90 91 112 111 
		f 4 -139 177 178 -176 mu 0 4 91 92 113 112 
		f 4 -141 179 180 -178 mu 0 4 92 93 114 113 
		f 4 -143 181 182 -180 mu 0 4 93 94 115 114 
		f 4 -145 183 184 -182 mu 0 4 94 95 116 115 
		f 4 -147 185 186 -184 mu 0 4 95 96 117 116 
		f 4 -149 187 188 -186 mu 0 4 96 97 118 117 
		f 4 -151 189 190 -188 mu 0 4 97 98 119 118 
		f 4 -153 191 192 -190 mu 0 4 98 99 120 119 
		f 4 -155 193 194 -192 mu 0 4 99 100 121 120 
		f 4 -157 195 196 -194 mu 0 4 100 101 122 121 
		f 4 -159 197 198 -196 mu 0 4 101 102 123 122 
		f 4 -160 -163 199 -198 mu 0 4 102 103 124 123 
		f 4 -162 200 201 202 mu 0 4 105 104 125 126 
		f 4 -165 203 204 -201 mu 0 4 104 106 127 125 
		f 4 -167 205 206 -204 mu 0 4 106 107 128 127 
		f 4 -169 207 208 -206 mu 0 4 107 108 129 128 
		f 4 -171 209 210 -208 mu 0 4 108 109 130 129 
		f 4 -173 211 212 -210 mu 0 4 109 110 131 130 
		f 4 -175 213 214 -212 mu 0 4 110 111 132 131 
		f 4 -177 215 216 -214 mu 0 4 111 112 133 132 
		f 4 -179 217 218 -216 mu 0 4 112 113 134 133 
		f 4 -181 219 220 -218 mu 0 4 113 114 135 134 
		f 4 -183 221 222 -220 mu 0 4 114 115 136 135 
		f 4 -185 223 224 -222 mu 0 4 115 116 137 136 
		f 4 -187 225 226 -224 mu 0 4 116 117 138 137 
		f 4 -189 227 228 -226 mu 0 4 117 118 139 138 
		f 4 -191 229 230 -228 mu 0 4 118 119 140 139 
		f 4 -193 231 232 -230 mu 0 4 119 120 141 140 
		f 4 -195 233 234 -232 mu 0 4 120 121 142 141 
		f 4 -197 235 236 -234 mu 0 4 121 122 143 142 
		f 4 -199 237 238 -236 mu 0 4 122 123 144 143 
		f 4 -200 -203 239 -238 mu 0 4 123 124 145 144 
		f 4 -202 240 241 242 mu 0 4 126 125 146 147 
		f 4 -205 243 244 -241 mu 0 4 125 127 148 146 
		f 4 -207 245 246 -244 mu 0 4 127 128 149 148 
		f 4 -209 247 248 -246 mu 0 4 128 129 150 149 
		f 4 -211 249 250 -248 mu 0 4 129 130 151 150 
		f 4 -213 251 252 -250 mu 0 4 130 131 152 151 
		f 4 -215 253 254 -252 mu 0 4 131 132 153 152 
		f 4 -217 255 256 -254 mu 0 4 132 133 154 153 
		f 4 -219 257 258 -256 mu 0 4 133 134 155 154 
		f 4 -221 259 260 -258 mu 0 4 134 135 156 155 
		f 4 -223 261 262 -260 mu 0 4 135 136 157 156 
		f 4 -225 263 264 -262 mu 0 4 136 137 158 157 
		f 4 -227 265 266 -264 mu 0 4 137 138 159 158 
		f 4 -229 267 268 -266 mu 0 4 138 139 160 159 
		f 4 -231 269 270 -268 mu 0 4 139 140 161 160 
		f 4 -233 271 272 -270 mu 0 4 140 141 162 161 
		f 4 -235 273 274 -272 mu 0 4 141 142 163 162 
		f 4 -237 275 276 -274 mu 0 4 142 143 164 163 
		f 4 -239 277 278 -276 mu 0 4 143 144 165 164 
		f 4 -240 -243 279 -278 mu 0 4 144 145 166 165 
		f 4 -242 280 281 282 mu 0 4 147 146 167 168 
		f 4 -245 283 284 -281 mu 0 4 146 148 169 167 
		f 4 -247 285 286 -284 mu 0 4 148 149 170 169 
		f 4 -249 287 288 -286 mu 0 4 149 150 171 170 
		f 4 -251 289 290 -288 mu 0 4 150 151 172 171 
		f 4 -253 291 292 -290 mu 0 4 151 152 173 172 
		f 4 -255 293 294 -292 mu 0 4 152 153 174 173 
		f 4 -257 295 296 -294 mu 0 4 153 154 175 174 
		f 4 -259 297 298 -296 mu 0 4 154 155 176 175 
		f 4 -261 299 300 -298 mu 0 4 155 156 177 176 
		f 4 -263 301 302 -300 mu 0 4 156 157 178 177 
		f 4 -265 303 304 -302 mu 0 4 157 158 179 178 
		f 4 -267 305 306 -304 mu 0 4 158 159 180 179 
		f 4 -269 307 308 -306 mu 0 4 159 160 181 180 
		f 4 -271 309 310 -308 mu 0 4 160 161 182 181 
		f 4 -273 311 312 -310 mu 0 4 161 162 183 182 
		f 4 -275 313 314 -312 mu 0 4 162 163 184 183 
		f 4 -277 315 316 -314 mu 0 4 163 164 185 184 
		f 4 -279 317 318 -316 mu 0 4 164 165 186 185 
		f 4 -280 -283 319 -318 mu 0 4 165 166 187 186 
		f 4 -282 320 321 322 mu 0 4 168 167 188 189 
		f 4 -285 323 324 -321 mu 0 4 167 169 190 188 
		f 4 -287 325 326 -324 mu 0 4 169 170 191 190 
		f 4 -289 327 328 -326 mu 0 4 170 171 192 191 
		f 4 -291 329 330 -328 mu 0 4 171 172 193 192 
		f 4 -293 331 332 -330 mu 0 4 172 173 194 193 
		f 4 -295 333 334 -332 mu 0 4 173 174 195 194 
		f 4 -297 335 336 -334 mu 0 4 174 175 196 195 
		f 4 -299 337 338 -336 mu 0 4 175 176 197 196 
		f 4 -301 339 340 -338 mu 0 4 176 177 198 197 
		f 4 -303 341 342 -340 mu 0 4 177 178 199 198 
		f 4 -305 343 344 -342 mu 0 4 178 179 200 199 
		f 4 -307 345 346 -344 mu 0 4 179 180 201 200 
		f 4 -309 347 348 -346 mu 0 4 180 181 202 201 
		f 4 -311 349 350 -348 mu 0 4 181 182 203 202 
		f 4 -313 351 352 -350 mu 0 4 182 183 204 203 
		f 4 -315 353 354 -352 mu 0 4 183 184 205 204 
		f 4 -317 355 356 -354 mu 0 4 184 185 206 205 
		f 4 -319 357 358 -356 mu 0 4 185 186 207 206 
		f 4 -320 -323 359 -358 mu 0 4 186 187 208 207 
		f 4 -322 360 361 362 mu 0 4 189 188 209 210 
		f 4 -325 363 364 -361 mu 0 4 188 190 211 209 
		f 4 -327 365 366 -364 mu 0 4 190 191 212 211 
		f 4 -329 367 368 -366 mu 0 4 191 192 213 212 
		f 4 -331 369 370 -368 mu 0 4 192 193 214 213 
		f 4 -333 371 372 -370 mu 0 4 193 194 215 214 
		f 4 -335 373 374 -372 mu 0 4 194 195 216 215 
		f 4 -337 375 376 -374 mu 0 4 195 196 217 216 
		f 4 -339 377 378 -376 mu 0 4 196 197 218 217 
		f 4 -341 379 380 -378 mu 0 4 197 198 219 218 
		f 4 -343 381 382 -380 mu 0 4 198 199 220 219 
		f 4 -345 383 384 -382 mu 0 4 199 200 221 220 
		f 4 -347 385 386 -384 mu 0 4 200 201 222 221 
		f 4 -349 387 388 -386 mu 0 4 201 202 223 222 
		f 4 -351 389 390 -388 mu 0 4 202 203 224 223 
		f 4 -353 391 392 -390 mu 0 4 203 204 225 224 
		f 4 -355 393 394 -392 mu 0 4 204 205 226 225 
		f 4 -357 395 396 -394 mu 0 4 205 206 227 226 
		f 4 -359 397 398 -396 mu 0 4 206 207 228 227 
		f 4 -360 -363 399 -398 mu 0 4 207 208 229 228 
		f 4 -362 400 401 402 mu 0 4 210 209 230 231 
		f 4 -365 403 404 -401 mu 0 4 209 211 232 230 
		f 4 -367 405 406 -404 mu 0 4 211 212 233 232 
		f 4 -369 407 408 -406 mu 0 4 212 213 234 233 
		f 4 -371 409 410 -408 mu 0 4 213 214 235 234 
		f 4 -373 411 412 -410 mu 0 4 214 215 236 235 
		f 4 -375 413 414 -412 mu 0 4 215 216 237 236 
		f 4 -377 415 416 -414 mu 0 4 216 217 238 237 
		f 4 -379 417 418 -416 mu 0 4 217 218 239 238 
		f 4 -381 419 420 -418 mu 0 4 218 219 240 239 
		f 4 -383 421 422 -420 mu 0 4 219 220 241 240 
		f 4 -385 423 424 -422 mu 0 4 220 221 242 241 
		f 4 -387 425 426 -424 mu 0 4 221 222 243 242 
		f 4 -389 427 428 -426 mu 0 4 222 223 244 243 
		f 4 -391 429 430 -428 mu 0 4 223 224 245 244 
		f 4 -393 431 432 -430 mu 0 4 224 225 246 245 
		f 4 -395 433 434 -432 mu 0 4 225 226 247 246 
		f 4 -397 435 436 -434 mu 0 4 226 227 248 247 
		f 4 -399 437 438 -436 mu 0 4 227 228 249 248 
		f 4 -400 -403 439 -438 mu 0 4 228 229 250 249 
		f 4 -402 440 441 442 mu 0 4 231 230 251 252 
		f 4 -405 443 444 -441 mu 0 4 230 232 253 251 
		f 4 -407 445 446 -444 mu 0 4 232 233 254 253 
		f 4 -409 447 448 -446 mu 0 4 233 234 255 254 
		f 4 -411 449 450 -448 mu 0 4 234 235 256 255 
		f 4 -413 451 452 -450 mu 0 4 235 236 257 256 
		f 4 -415 453 454 -452 mu 0 4 236 237 258 257 
		f 4 -417 455 456 -454 mu 0 4 237 238 259 258 
		f 4 -419 457 458 -456 mu 0 4 238 239 260 259 
		f 4 -421 459 460 -458 mu 0 4 239 240 261 260 
		f 4 -423 461 462 -460 mu 0 4 240 241 262 261 
		f 4 -425 463 464 -462 mu 0 4 241 242 263 262 
		f 4 -427 465 466 -464 mu 0 4 242 243 264 263 
		f 4 -429 467 468 -466 mu 0 4 243 244 265 264 
		f 4 -431 469 470 -468 mu 0 4 244 245 266 265 
		f 4 -433 471 472 -470 mu 0 4 245 246 267 266 
		f 4 -435 473 474 -472 mu 0 4 246 247 268 267 
		f 4 -437 475 476 -474 mu 0 4 247 248 269 268 
		f 4 -439 477 478 -476 mu 0 4 248 249 270 269 
		f 4 -440 -443 479 -478 mu 0 4 249 250 271 270 
		f 4 -442 480 481 482 mu 0 4 252 251 272 273 
		f 4 -445 483 484 -481 mu 0 4 251 253 274 272 
		f 4 -447 485 486 -484 mu 0 4 253 254 275 274 
		f 4 -449 487 488 -486 mu 0 4 254 255 276 275 
		f 4 -451 489 490 -488 mu 0 4 255 256 277 276 
		f 4 -453 491 492 -490 mu 0 4 256 257 278 277 
		f 4 -455 493 494 -492 mu 0 4 257 258 279 278 
		f 4 -457 495 496 -494 mu 0 4 258 259 280 279 
		f 4 -459 497 498 -496 mu 0 4 259 260 281 280 
		f 4 -461 499 500 -498 mu 0 4 260 261 282 281 ;
	setAttr ".fc[250:399]" -type "polyFaces" 
		f 4 -463 501 502 -500 mu 0 4 261 262 283 282 
		f 4 -465 503 504 -502 mu 0 4 262 263 284 283 
		f 4 -467 505 506 -504 mu 0 4 263 264 285 284 
		f 4 -469 507 508 -506 mu 0 4 264 265 286 285 
		f 4 -471 509 510 -508 mu 0 4 265 266 287 286 
		f 4 -473 511 512 -510 mu 0 4 266 267 288 287 
		f 4 -475 513 514 -512 mu 0 4 267 268 289 288 
		f 4 -477 515 516 -514 mu 0 4 268 269 290 289 
		f 4 -479 517 518 -516 mu 0 4 269 270 291 290 
		f 4 -480 -483 519 -518 mu 0 4 270 271 292 291 
		f 4 -482 520 521 522 mu 0 4 273 272 293 294 
		f 4 -485 523 524 -521 mu 0 4 272 274 295 293 
		f 4 -487 525 526 -524 mu 0 4 274 275 296 295 
		f 4 -489 527 528 -526 mu 0 4 275 276 297 296 
		f 4 -491 529 530 -528 mu 0 4 276 277 298 297 
		f 4 -493 531 532 -530 mu 0 4 277 278 299 298 
		f 4 -495 533 534 -532 mu 0 4 278 279 300 299 
		f 4 -497 535 536 -534 mu 0 4 279 280 301 300 
		f 4 -499 537 538 -536 mu 0 4 280 281 302 301 
		f 4 -501 539 540 -538 mu 0 4 281 282 303 302 
		f 4 -503 541 542 -540 mu 0 4 282 283 304 303 
		f 4 -505 543 544 -542 mu 0 4 283 284 305 304 
		f 4 -507 545 546 -544 mu 0 4 284 285 306 305 
		f 4 -509 547 548 -546 mu 0 4 285 286 307 306 
		f 4 -511 549 550 -548 mu 0 4 286 287 308 307 
		f 4 -513 551 552 -550 mu 0 4 287 288 309 308 
		f 4 -515 553 554 -552 mu 0 4 288 289 310 309 
		f 4 -517 555 556 -554 mu 0 4 289 290 311 310 
		f 4 -519 557 558 -556 mu 0 4 290 291 312 311 
		f 4 -520 -523 559 -558 mu 0 4 291 292 313 312 
		f 4 -522 560 561 562 mu 0 4 294 293 314 315 
		f 4 -525 563 564 -561 mu 0 4 293 295 316 314 
		f 4 -527 565 566 -564 mu 0 4 295 296 317 316 
		f 4 -529 567 568 -566 mu 0 4 296 297 318 317 
		f 4 -531 569 570 -568 mu 0 4 297 298 319 318 
		f 4 -533 571 572 -570 mu 0 4 298 299 320 319 
		f 4 -535 573 574 -572 mu 0 4 299 300 321 320 
		f 4 -537 575 576 -574 mu 0 4 300 301 322 321 
		f 4 -539 577 578 -576 mu 0 4 301 302 323 322 
		f 4 -541 579 580 -578 mu 0 4 302 303 324 323 
		f 4 -543 581 582 -580 mu 0 4 303 304 325 324 
		f 4 -545 583 584 -582 mu 0 4 304 305 326 325 
		f 4 -547 585 586 -584 mu 0 4 305 306 327 326 
		f 4 -549 587 588 -586 mu 0 4 306 307 328 327 
		f 4 -551 589 590 -588 mu 0 4 307 308 329 328 
		f 4 -553 591 592 -590 mu 0 4 308 309 330 329 
		f 4 -555 593 594 -592 mu 0 4 309 310 331 330 
		f 4 -557 595 596 -594 mu 0 4 310 311 332 331 
		f 4 -559 597 598 -596 mu 0 4 311 312 333 332 
		f 4 -560 -563 599 -598 mu 0 4 312 313 334 333 
		f 4 -562 600 601 602 mu 0 4 315 314 335 336 
		f 4 -565 603 604 -601 mu 0 4 314 316 337 335 
		f 4 -567 605 606 -604 mu 0 4 316 317 338 337 
		f 4 -569 607 608 -606 mu 0 4 317 318 339 338 
		f 4 -571 609 610 -608 mu 0 4 318 319 340 339 
		f 4 -573 611 612 -610 mu 0 4 319 320 341 340 
		f 4 -575 613 614 -612 mu 0 4 320 321 342 341 
		f 4 -577 615 616 -614 mu 0 4 321 322 343 342 
		f 4 -579 617 618 -616 mu 0 4 322 323 344 343 
		f 4 -581 619 620 -618 mu 0 4 323 324 345 344 
		f 4 -583 621 622 -620 mu 0 4 324 325 346 345 
		f 4 -585 623 624 -622 mu 0 4 325 326 347 346 
		f 4 -587 625 626 -624 mu 0 4 326 327 348 347 
		f 4 -589 627 628 -626 mu 0 4 327 328 349 348 
		f 4 -591 629 630 -628 mu 0 4 328 329 350 349 
		f 4 -593 631 632 -630 mu 0 4 329 330 351 350 
		f 4 -595 633 634 -632 mu 0 4 330 331 352 351 
		f 4 -597 635 636 -634 mu 0 4 331 332 353 352 
		f 4 -599 637 638 -636 mu 0 4 332 333 354 353 
		f 4 -600 -603 639 -638 mu 0 4 333 334 355 354 
		f 4 -602 640 641 642 mu 0 4 336 335 356 357 
		f 4 -605 643 644 -641 mu 0 4 335 337 358 356 
		f 4 -607 645 646 -644 mu 0 4 337 338 359 358 
		f 4 -609 647 648 -646 mu 0 4 338 339 360 359 
		f 4 -611 649 650 -648 mu 0 4 339 340 361 360 
		f 4 -613 651 652 -650 mu 0 4 340 341 362 361 
		f 4 -615 653 654 -652 mu 0 4 341 342 363 362 
		f 4 -617 655 656 -654 mu 0 4 342 343 364 363 
		f 4 -619 657 658 -656 mu 0 4 343 344 365 364 
		f 4 -621 659 660 -658 mu 0 4 344 345 366 365 
		f 4 -623 661 662 -660 mu 0 4 345 346 367 366 
		f 4 -625 663 664 -662 mu 0 4 346 347 368 367 
		f 4 -627 665 666 -664 mu 0 4 347 348 369 368 
		f 4 -629 667 668 -666 mu 0 4 348 349 370 369 
		f 4 -631 669 670 -668 mu 0 4 349 350 371 370 
		f 4 -633 671 672 -670 mu 0 4 350 351 372 371 
		f 4 -635 673 674 -672 mu 0 4 351 352 373 372 
		f 4 -637 675 676 -674 mu 0 4 352 353 374 373 
		f 4 -639 677 678 -676 mu 0 4 353 354 375 374 
		f 4 -640 -643 679 -678 mu 0 4 354 355 376 375 
		f 4 -642 680 681 682 mu 0 4 357 356 377 378 
		f 4 -645 683 684 -681 mu 0 4 356 358 379 377 
		f 4 -647 685 686 -684 mu 0 4 358 359 380 379 
		f 4 -649 687 688 -686 mu 0 4 359 360 381 380 
		f 4 -651 689 690 -688 mu 0 4 360 361 382 381 
		f 4 -653 691 692 -690 mu 0 4 361 362 383 382 
		f 4 -655 693 694 -692 mu 0 4 362 363 384 383 
		f 4 -657 695 696 -694 mu 0 4 363 364 385 384 
		f 4 -659 697 698 -696 mu 0 4 364 365 386 385 
		f 4 -661 699 700 -698 mu 0 4 365 366 387 386 
		f 4 -663 701 702 -700 mu 0 4 366 367 388 387 
		f 4 -665 703 704 -702 mu 0 4 367 368 389 388 
		f 4 -667 705 706 -704 mu 0 4 368 369 390 389 
		f 4 -669 707 708 -706 mu 0 4 369 370 391 390 
		f 4 -671 709 710 -708 mu 0 4 370 371 392 391 
		f 4 -673 711 712 -710 mu 0 4 371 372 393 392 
		f 4 -675 713 714 -712 mu 0 4 372 373 394 393 
		f 4 -677 715 716 -714 mu 0 4 373 374 395 394 
		f 4 -679 717 718 -716 mu 0 4 374 375 396 395 
		f 4 -680 -683 719 -718 mu 0 4 375 376 397 396 
		f 4 -682 720 721 722 mu 0 4 378 377 398 399 
		f 4 -685 723 724 -721 mu 0 4 377 379 400 398 
		f 4 -687 725 726 -724 mu 0 4 379 380 401 400 
		f 4 -689 727 728 -726 mu 0 4 380 381 402 401 
		f 4 -691 729 730 -728 mu 0 4 381 382 403 402 
		f 4 -693 731 732 -730 mu 0 4 382 383 404 403 
		f 4 -695 733 734 -732 mu 0 4 383 384 405 404 
		f 4 -697 735 736 -734 mu 0 4 384 385 406 405 
		f 4 -699 737 738 -736 mu 0 4 385 386 407 406 
		f 4 -701 739 740 -738 mu 0 4 386 387 408 407 
		f 4 -703 741 742 -740 mu 0 4 387 388 409 408 
		f 4 -705 743 744 -742 mu 0 4 388 389 410 409 
		f 4 -707 745 746 -744 mu 0 4 389 390 411 410 
		f 4 -709 747 748 -746 mu 0 4 390 391 412 411 
		f 4 -711 749 750 -748 mu 0 4 391 392 413 412 
		f 4 -713 751 752 -750 mu 0 4 392 393 414 413 
		f 4 -715 753 754 -752 mu 0 4 393 394 415 414 
		f 4 -717 755 756 -754 mu 0 4 394 395 416 415 
		f 4 -719 757 758 -756 mu 0 4 395 396 417 416 
		f 4 -720 -723 759 -758 mu 0 4 396 397 418 417 
		f 3 -722 760 761 mu 0 3 399 398 419 
		f 3 -725 762 -761 mu 0 3 398 400 420 
		f 3 -727 763 -763 mu 0 3 400 401 421 
		f 3 -729 764 -764 mu 0 3 401 402 422 
		f 3 -731 765 -765 mu 0 3 402 403 423 
		f 3 -733 766 -766 mu 0 3 403 404 424 
		f 3 -735 767 -767 mu 0 3 404 405 425 
		f 3 -737 768 -768 mu 0 3 405 406 426 
		f 3 -739 769 -769 mu 0 3 406 407 427 
		f 3 -741 770 -770 mu 0 3 407 408 428 
		f 3 -743 771 -771 mu 0 3 408 409 429 
		f 3 -745 772 -772 mu 0 3 409 410 430 
		f 3 -747 773 -773 mu 0 3 410 411 431 
		f 3 -749 774 -774 mu 0 3 411 412 432 
		f 3 -751 775 -775 mu 0 3 412 413 433 
		f 3 -753 776 -776 mu 0 3 413 414 434 
		f 3 -755 777 -777 mu 0 3 414 415 435 
		f 3 -757 778 -778 mu 0 3 415 416 436 
		f 3 -759 779 -779 mu 0 3 416 417 437 
		f 3 -760 -762 -780 mu 0 3 417 418 438 ;
createNode transform -n "keyLight";
	setAttr ".r" -type "double3" -45 35 0 ;
createNode directionalLight -n "keyLightShape" -p "keyLight";
	setAttr -k off ".v";
createNode transform -n "fillLight";
createNode ambientLight -n "fillLightShape" -p "fillLight";
	setAttr -k off ".v";
	setAttr ".in" 0.25;
createNode lambert -n "floorMat";
	setAttr ".c" -type "float3" 0.45 0.45 0.45 ;
createNode shadingEngine -n "floorMatSG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody1";
	setAttr ".object" -type "string" "floor";
	setAttr ".active" no;
createNode rigidBody -n "rigidBody2";
	setAttr ".object" -type "string" "block_0_0";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 0 22.5 -51 1;
createNode lambert -n "blockMat00";
	setAttr ".c" -type "float3" 0.8412068543778499 0.4 0.25 ;
createNode shadingEngine -n "blockMat00SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody3";
	setAttr ".object" -type "string" "block_0_1";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 0 22.5 0 1;
createNode lambert -n "blockMat01";
	setAttr ".c" -type "float3" 0.8395654974118699 0.4 0.25 ;
createNode shadingEngine -n "blockMat01SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody4";
	setAttr ".object" -type "string" "block_0_2";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 0 22.5 51 1;
createNode lambert -n "blockMat02";
	setAttr ".c" -type "float3" 0.6613102735453618 0.4 0.25 ;
createNode shadingEngine -n "blockMat02SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody5";
	setAttr ".object" -type "string" "block_1_0";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 -51 67.5 0 1;
createNode lambert -n "blockMat10";
	setAttr ".c" -type "float3" 0.6669743990317843 0.4 0.25 ;
createNode shadingEngine -n "blockMat10SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody6";
	setAttr ".object" -type "string" "block_1_1";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 0 67.5 0 1;
createNode lambert -n "blockMat11";
	setAttr ".c" -type "float3" 0.81709977562589 0.4 0.25 ;
createNode shadingEngine -n "blockMat11SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody7";
	setAttr ".object" -type "string" "block_1_2";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 51 67.5 0 1;
createNode lambert -n "blockMat12";
	setAttr ".c" -type "float3" 0.7971939978137046 0.4 0.25 ;
createNode shadingEngine -n "blockMat12SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody8";
	setAttr ".object" -type "string" "block_2_0";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 0 112.5 -51 1;
createNode lambert -n "blockMat20";
	setAttr ".c" -type "float3" 0.7839460802880442 0.4 0.25 ;
createNode shadingEngine -n "blockMat20SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody9";
	setAttr ".object" -type "string" "block_2_1";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 0 112.5 0 1;
createNode lambert -n "blockMat21";
	setAttr ".c" -type "float3" 0.7116272915178289 0.4 0.25 ;
createNode shadingEngine -n "blockMat21SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody10";
	setAttr ".object" -type "string" "block_2_2";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 0 112.5 51 1;
createNode lambert -n "blockMat22";
	setAttr ".c" -type "float3" 0.7711888331356925 0.4 0.25 ;
createNode shadingEngine -n "blockMat22SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody11";
	setAttr ".object" -type "string" "block_3_0";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 -51 157.5 0 1;
createNode lambert -n "blockMat30";
	setAttr ".c" -type "float3" 0.7713603467281676 0.4 0.25 ;
createNode shadingEngine -n "blockMat30SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody12";
	setAttr ".object" -type "string" "block_3_1";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 0 157.5 0 1;
createNode lambert -n "blockMat31";
	setAttr ".c" -type "float3" 0.7662408034224006 0.4 0.25 ;
createNode shadingEngine -n "blockMat31SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody13";
	setAttr ".object" -type "string" "block_3_2";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 51 157.5 0 1;
createNode lambert -n "blockMat32";
	setAttr ".c" -type "float3" 0.6816765740509612 0.4 0.25 ;
createNode shadingEngine -n "blockMat32SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody14";
	setAttr ".object" -type "string" "block_4_0";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 0 202.5 -51 1;
createNode lambert -n "blockMat40";
	setAttr ".c" -type "float3" 0.7361339280582537 0.4 0.25 ;
createNode shadingEngine -n "blockMat40SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody15";
	setAttr ".object" -type "string" "block_4_1";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 0 202.5 0 1;
createNode lambert -n "blockMat41";
	setAttr ".c" -type "float3" 0.7287063640410743 0.4 0.25 ;
createNode shadingEngine -n "blockMat41SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody16";
	setAttr ".object" -type "string" "block_4_2";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 0 202.5 51 1;
createNode lambert -n "blockMat42";
	setAttr ".c" -type "float3" 0.7946024162474932 0.4 0.25 ;
createNode shadingEngine -n "blockMat42SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody17";
	setAttr ".object" -type "string" "block_5_0";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 -51 247.5 0 1;
createNode lambert -n "blockMat50";
	setAttr ".c" -type "float3" 0.8489639125899485 0.4 0.25 ;
createNode shadingEngine -n "blockMat50SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody18";
	setAttr ".object" -type "string" "block_5_1";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 0 247.5 0 1;
createNode lambert -n "blockMat51";
	setAttr ".c" -type "float3" 0.8398790946186487 0.4 0.25 ;
createNode shadingEngine -n "blockMat51SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody19";
	setAttr ".object" -type "string" "block_5_2";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 51 247.5 0 1;
createNode lambert -n "blockMat52";
	setAttr ".c" -type "float3" 0.7588354094858641 0.4 0.25 ;
createNode shadingEngine -n "blockMat52SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody20";
	setAttr ".object" -type "string" "block_6_0";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 0 292.5 -51 1;
createNode lambert -n "blockMat60";
	setAttr ".c" -type "float3" 0.7389708377451707 0.4 0.25 ;
createNode shadingEngine -n "blockMat60SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody21";
	setAttr ".object" -type "string" "block_6_1";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 0 292.5 0 1;
createNode lambert -n "blockMat61";
	setAttr ".c" -type "float3" 0.7036481483298657 0.4 0.25 ;
createNode shadingEngine -n "blockMat61SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody22";
	setAttr ".object" -type "string" "block_6_2";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 0 292.5 51 1;
createNode lambert -n "blockMat62";
	setAttr ".c" -type "float3" 0.6571848658785715 0.4 0.25 ;
createNode shadingEngine -n "blockMat62SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody23";
	setAttr ".object" -type "string" "block_7_0";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 -51 337.5 0 1;
createNode lambert -n "blockMat70";
	setAttr ".c" -type "float3" 0.6554889714181638 0.4 0.25 ;
createNode shadingEngine -n "blockMat70SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody24";
	setAttr ".object" -type "string" "block_7_1";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 0 337.5 0 1;
createNode lambert -n "blockMat71";
	setAttr ".c" -type "float3" 0.7429787724194624 0.4 0.25 ;
createNode shadingEngine -n "blockMat71SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody25";
	setAttr ".object" -type "string" "block_7_2";
	setAttr ".mass" 40;
	setAttr ".bounciness" 0.1;
	setAttr ".staticFriction" 0.6;
	setAttr ".dynamicFriction" 0.5;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 51 337.5 0 1;
createNode lambert -n "blockMat72";
	setAttr ".c" -type "float3" 0.7136930255707354 0.4 0.25 ;
createNode shadingEngine -n "blockMat72SG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode rigidBody -n "rigidBody26";
	setAttr ".object" -type "string" "wreckingBall";
	setAttr ".mass" 800;
	setAttr ".initialVelocity" -type "double3" 1600 0 0 ;
	setAttr ".initialMatrix" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 -700 120 0 1;
createNode blinn -n "ballMat";
	setAttr ".c" -type "float3" 0.15 0.15 0.18 ;
createNode shadingEngine -n "ballMatSG";
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
	setAttr ".b" -type "string" "playbackOptions -min 1 -max 150 -ast 1 -aet 240 ";
	setAttr ".st" 6;
select -ne :time1;
	setAttr ".o" 1;
	setAttr ".unw" 1;
relationship "link" ":lightLinker1" ":initialShadingGroup.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "floorMatSG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat00SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat01SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat02SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat10SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat11SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat12SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat20SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat21SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat22SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat30SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat31SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat32SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat40SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat41SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat42SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat50SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat51SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat52SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat60SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat61SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat62SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat70SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat71SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "blockMat72SG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "ballMatSG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" ":initialShadingGroup.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "floorMatSG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat00SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat01SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat02SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat10SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat11SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat12SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat20SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat21SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat22SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat30SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat31SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat32SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat40SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat41SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat42SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat50SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat51SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat52SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat60SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat61SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat62SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat70SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat71SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "blockMat72SG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "ballMatSG.message" ":defaultLightSet.message";
connectAttr "ballMat.oc" "ballMatSG.ss";
connectAttr "blockMat00.oc" "blockMat00SG.ss";
connectAttr "blockMat01.oc" "blockMat01SG.ss";
connectAttr "blockMat02.oc" "blockMat02SG.ss";
connectAttr "blockMat10.oc" "blockMat10SG.ss";
connectAttr "blockMat11.oc" "blockMat11SG.ss";
connectAttr "blockMat12.oc" "blockMat12SG.ss";
connectAttr "blockMat20.oc" "blockMat20SG.ss";
connectAttr "blockMat21.oc" "blockMat21SG.ss";
connectAttr "blockMat22.oc" "blockMat22SG.ss";
connectAttr "blockMat30.oc" "blockMat30SG.ss";
connectAttr "blockMat31.oc" "blockMat31SG.ss";
connectAttr "blockMat32.oc" "blockMat32SG.ss";
connectAttr "blockMat40.oc" "blockMat40SG.ss";
connectAttr "blockMat41.oc" "blockMat41SG.ss";
connectAttr "blockMat42.oc" "blockMat42SG.ss";
connectAttr "blockMat50.oc" "blockMat50SG.ss";
connectAttr "blockMat51.oc" "blockMat51SG.ss";
connectAttr "blockMat52.oc" "blockMat52SG.ss";
connectAttr "blockMat60.oc" "blockMat60SG.ss";
connectAttr "blockMat61.oc" "blockMat61SG.ss";
connectAttr "blockMat62.oc" "blockMat62SG.ss";
connectAttr "blockMat70.oc" "blockMat70SG.ss";
connectAttr "blockMat71.oc" "blockMat71SG.ss";
connectAttr "blockMat72.oc" "blockMat72SG.ss";
connectAttr "floorMat.oc" "floorMatSG.ss";
connectAttr "rigidBody10.outRotate" "block_2_2.rotate";
connectAttr "rigidBody10.outTranslate" "block_2_2.translate";
connectAttr "rigidBody11.outRotate" "block_3_0.rotate";
connectAttr "rigidBody11.outTranslate" "block_3_0.translate";
connectAttr "rigidBody12.outRotate" "block_3_1.rotate";
connectAttr "rigidBody12.outTranslate" "block_3_1.translate";
connectAttr "rigidBody13.outRotate" "block_3_2.rotate";
connectAttr "rigidBody13.outTranslate" "block_3_2.translate";
connectAttr "rigidBody14.outRotate" "block_4_0.rotate";
connectAttr "rigidBody14.outTranslate" "block_4_0.translate";
connectAttr "rigidBody15.outRotate" "block_4_1.rotate";
connectAttr "rigidBody15.outTranslate" "block_4_1.translate";
connectAttr "rigidBody16.outRotate" "block_4_2.rotate";
connectAttr "rigidBody16.outTranslate" "block_4_2.translate";
connectAttr "rigidBody17.outRotate" "block_5_0.rotate";
connectAttr "rigidBody17.outTranslate" "block_5_0.translate";
connectAttr "rigidBody18.outRotate" "block_5_1.rotate";
connectAttr "rigidBody18.outTranslate" "block_5_1.translate";
connectAttr "rigidBody19.outRotate" "block_5_2.rotate";
connectAttr "rigidBody19.outTranslate" "block_5_2.translate";
connectAttr "rigidBody2.outRotate" "block_0_0.rotate";
connectAttr "rigidBody2.outTranslate" "block_0_0.translate";
connectAttr "rigidBody20.outRotate" "block_6_0.rotate";
connectAttr "rigidBody20.outTranslate" "block_6_0.translate";
connectAttr "rigidBody21.outRotate" "block_6_1.rotate";
connectAttr "rigidBody21.outTranslate" "block_6_1.translate";
connectAttr "rigidBody22.outRotate" "block_6_2.rotate";
connectAttr "rigidBody22.outTranslate" "block_6_2.translate";
connectAttr "rigidBody23.outRotate" "block_7_0.rotate";
connectAttr "rigidBody23.outTranslate" "block_7_0.translate";
connectAttr "rigidBody24.outRotate" "block_7_1.rotate";
connectAttr "rigidBody24.outTranslate" "block_7_1.translate";
connectAttr "rigidBody25.outRotate" "block_7_2.rotate";
connectAttr "rigidBody25.outTranslate" "block_7_2.translate";
connectAttr "rigidBody26.outRotate" "wreckingBall.rotate";
connectAttr "rigidBody26.outTranslate" "wreckingBall.translate";
connectAttr "rigidBody3.outRotate" "block_0_1.rotate";
connectAttr "rigidBody3.outTranslate" "block_0_1.translate";
connectAttr "rigidBody4.outRotate" "block_0_2.rotate";
connectAttr "rigidBody4.outTranslate" "block_0_2.translate";
connectAttr "rigidBody5.outRotate" "block_1_0.rotate";
connectAttr "rigidBody5.outTranslate" "block_1_0.translate";
connectAttr "rigidBody6.outRotate" "block_1_1.rotate";
connectAttr "rigidBody6.outTranslate" "block_1_1.translate";
connectAttr "rigidBody7.outRotate" "block_1_2.rotate";
connectAttr "rigidBody7.outTranslate" "block_1_2.translate";
connectAttr "rigidBody8.outRotate" "block_2_0.rotate";
connectAttr "rigidBody8.outTranslate" "block_2_0.translate";
connectAttr "rigidBody9.outRotate" "block_2_1.rotate";
connectAttr "rigidBody9.outTranslate" "block_2_1.translate";
connectAttr "solverShape1.message" "rigidBody1.solver";
connectAttr "solverShape1.message" "rigidBody10.solver";
connectAttr "solverShape1.message" "rigidBody11.solver";
connectAttr "solverShape1.message" "rigidBody12.solver";
connectAttr "solverShape1.message" "rigidBody13.solver";
connectAttr "solverShape1.message" "rigidBody14.solver";
connectAttr "solverShape1.message" "rigidBody15.solver";
connectAttr "solverShape1.message" "rigidBody16.solver";
connectAttr "solverShape1.message" "rigidBody17.solver";
connectAttr "solverShape1.message" "rigidBody18.solver";
connectAttr "solverShape1.message" "rigidBody19.solver";
connectAttr "solverShape1.message" "rigidBody2.solver";
connectAttr "solverShape1.message" "rigidBody20.solver";
connectAttr "solverShape1.message" "rigidBody21.solver";
connectAttr "solverShape1.message" "rigidBody22.solver";
connectAttr "solverShape1.message" "rigidBody23.solver";
connectAttr "solverShape1.message" "rigidBody24.solver";
connectAttr "solverShape1.message" "rigidBody25.solver";
connectAttr "solverShape1.message" "rigidBody26.solver";
connectAttr "solverShape1.message" "rigidBody3.solver";
connectAttr "solverShape1.message" "rigidBody4.solver";
connectAttr "solverShape1.message" "rigidBody5.solver";
connectAttr "solverShape1.message" "rigidBody6.solver";
connectAttr "solverShape1.message" "rigidBody7.solver";
connectAttr "solverShape1.message" "rigidBody8.solver";
connectAttr "solverShape1.message" "rigidBody9.solver";
connectAttr "ballMat.msg" ":defaultShaderList1.s" -na;
connectAttr "ballMatSG.pa" ":renderPartition.st" -na;
connectAttr "blockMat00.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat00SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat01.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat01SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat02.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat02SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat10.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat10SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat11.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat11SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat12.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat12SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat20.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat20SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat21.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat21SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat22.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat22SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat30.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat30SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat31.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat31SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat32.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat32SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat40.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat40SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat41.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat41SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat42.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat42SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat50.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat50SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat51.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat51SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat52.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat52SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat60.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat60SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat61.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat61SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat62.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat62SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat70.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat70SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat71.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat71SG.pa" ":renderPartition.st" -na;
connectAttr "blockMat72.msg" ":defaultShaderList1.s" -na;
connectAttr "blockMat72SG.pa" ":renderPartition.st" -na;
connectAttr "block_0_Shape0.iog" "blockMat00SG.dsm" -na;
connectAttr "block_0_Shape1.iog" "blockMat01SG.dsm" -na;
connectAttr "block_0_Shape2.iog" "blockMat02SG.dsm" -na;
connectAttr "block_1_Shape0.iog" "blockMat10SG.dsm" -na;
connectAttr "block_1_Shape1.iog" "blockMat11SG.dsm" -na;
connectAttr "block_1_Shape2.iog" "blockMat12SG.dsm" -na;
connectAttr "block_2_Shape0.iog" "blockMat20SG.dsm" -na;
connectAttr "block_2_Shape1.iog" "blockMat21SG.dsm" -na;
connectAttr "block_2_Shape2.iog" "blockMat22SG.dsm" -na;
connectAttr "block_3_Shape0.iog" "blockMat30SG.dsm" -na;
connectAttr "block_3_Shape1.iog" "blockMat31SG.dsm" -na;
connectAttr "block_3_Shape2.iog" "blockMat32SG.dsm" -na;
connectAttr "block_4_Shape0.iog" "blockMat40SG.dsm" -na;
connectAttr "block_4_Shape1.iog" "blockMat41SG.dsm" -na;
connectAttr "block_4_Shape2.iog" "blockMat42SG.dsm" -na;
connectAttr "block_5_Shape0.iog" "blockMat50SG.dsm" -na;
connectAttr "block_5_Shape1.iog" "blockMat51SG.dsm" -na;
connectAttr "block_5_Shape2.iog" "blockMat52SG.dsm" -na;
connectAttr "block_6_Shape0.iog" "blockMat60SG.dsm" -na;
connectAttr "block_6_Shape1.iog" "blockMat61SG.dsm" -na;
connectAttr "block_6_Shape2.iog" "blockMat62SG.dsm" -na;
connectAttr "block_7_Shape0.iog" "blockMat70SG.dsm" -na;
connectAttr "block_7_Shape1.iog" "blockMat71SG.dsm" -na;
connectAttr "block_7_Shape2.iog" "blockMat72SG.dsm" -na;
connectAttr "fillLight.iog" ":defaultLightSet.dsm" -na;
connectAttr "fillLightShape.ltd" ":lightList1.l" -na;
connectAttr "floorMat.msg" ":defaultShaderList1.s" -na;
connectAttr "floorMatSG.pa" ":renderPartition.st" -na;
connectAttr "floorShape.iog" "floorMatSG.dsm" -na;
connectAttr "keyLight.iog" ":defaultLightSet.dsm" -na;
connectAttr "keyLightShape.ltd" ":lightList1.l" -na;
connectAttr "wreckingBallShape.iog" "ballMatSG.dsm" -na;
// End of rigid_tower.ma
