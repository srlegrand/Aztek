//Maya ASCII 2024 scene
//Name: depth_of_field.ma
//Codeset: UTF-8
requires maya "2024";
currentUnit -l centimeter -a degree -t film;
fileInfo "application" "aztek";
fileInfo "version" "1.0.0";
createNode transform -s -n "persp";
	setAttr ".v" no;
	setAttr ".t" -type "double3" 6 4 12 ;
	setAttr ".r" -type "double3" -9.957470402363633 20.556045219583467 0 ;
createNode camera -s -n "perspShape" -p "persp";
	setAttr ".coi" 17.349351572897472;
	setAttr ".rnd" no;
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
createNode transform -n "post0";
	setAttr ".t" -type "double3" -2 1 4 ;
createNode mesh -n "postShape0" -p "post0";
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
	setAttr ".vt[0:7]"  -0.3 -1 0.3 0.3 -1 0.3 0.3 1 0.3 -0.3 1 0.3 0.3 1 -0.3 -0.3 1 -0.3 0.3 -1 -0.3 -0.3 -1 -0.3;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "post1";
	setAttr ".t" -type "double3" -1.5 1 1.7999999999999998 ;
createNode mesh -n "postShape1" -p "post1";
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
	setAttr ".vt[0:7]"  -0.3 -1 0.3 0.3 -1 0.3 0.3 1 0.3 -0.3 1 0.3 0.3 1 -0.3 -0.3 1 -0.3 0.3 -1 -0.3 -0.3 -1 -0.3;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "post2";
	setAttr ".t" -type "double3" -1 1 -0.40000000000000036 ;
createNode mesh -n "postShape2" -p "post2";
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
	setAttr ".vt[0:7]"  -0.3 -1 0.3 0.3 -1 0.3 0.3 1 0.3 -0.3 1 0.3 0.3 1 -0.3 -0.3 1 -0.3 0.3 -1 -0.3 -0.3 -1 -0.3;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "post3";
	setAttr ".t" -type "double3" -0.5 1 -2.6000000000000005 ;
createNode mesh -n "postShape3" -p "post3";
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
	setAttr ".vt[0:7]"  -0.3 -1 0.3 0.3 -1 0.3 0.3 1 0.3 -0.3 1 0.3 0.3 1 -0.3 -0.3 1 -0.3 0.3 -1 -0.3 -0.3 -1 -0.3;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "post4";
	setAttr ".t" -type "double3" 0 1 -4.800000000000001 ;
createNode mesh -n "postShape4" -p "post4";
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
	setAttr ".vt[0:7]"  -0.3 -1 0.3 0.3 -1 0.3 0.3 1 0.3 -0.3 1 0.3 0.3 1 -0.3 -0.3 1 -0.3 0.3 -1 -0.3 -0.3 -1 -0.3;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "post5";
	setAttr ".t" -type "double3" 0.5 1 -7 ;
createNode mesh -n "postShape5" -p "post5";
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
	setAttr ".vt[0:7]"  -0.3 -1 0.3 0.3 -1 0.3 0.3 1 0.3 -0.3 1 0.3 0.3 1 -0.3 -0.3 1 -0.3 0.3 -1 -0.3 -0.3 -1 -0.3;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "post6";
	setAttr ".t" -type "double3" 1 1 -9.200000000000001 ;
createNode mesh -n "postShape6" -p "post6";
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
	setAttr ".vt[0:7]"  -0.3 -1 0.3 0.3 -1 0.3 0.3 1 0.3 -0.3 1 0.3 0.3 1 -0.3 -0.3 1 -0.3 0.3 -1 -0.3 -0.3 -1 -0.3;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "post7";
	setAttr ".t" -type "double3" 1.5 1 -11.400000000000002 ;
createNode mesh -n "postShape7" -p "post7";
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
	setAttr ".vt[0:7]"  -0.3 -1 0.3 0.3 -1 0.3 0.3 1 0.3 -0.3 1 0.3 0.3 1 -0.3 -0.3 1 -0.3 0.3 -1 -0.3 -0.3 -1 -0.3;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "post8";
	setAttr ".t" -type "double3" 2 1 -13.600000000000001 ;
createNode mesh -n "postShape8" -p "post8";
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
	setAttr ".vt[0:7]"  -0.3 -1 0.3 0.3 -1 0.3 0.3 1 0.3 -0.3 1 0.3 0.3 1 -0.3 -0.3 1 -0.3 0.3 -1 -0.3 -0.3 -1 -0.3;
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
	setAttr ".vt[0:3]"  -30 0 30 30 0 30 -30 0 -30 30 0 -30;
	setAttr -s 4 ".ed";
	setAttr ".ed[0:3]"  0 1 1 1 3 1 3 2 1 2 0 1;
	setAttr -s 1 -ch 4 ".fc[0]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 ;
createNode transform -n "sky";
createNode aiSkyDomeLight -n "skyShape" -p "sky";
	setAttr -k off ".v";
	setAttr ".in" 0.8;
createNode transform -n "shotCam";
	setAttr ".t" -type "double3" 2.5 1.6 9 ;
	setAttr ".r" -type "double3" -3.012324816539346 15.255118703057777 0 ;
createNode camera -n "shotCamShape" -p "shotCam";
	setAttr -k off ".v";
	setAttr ".fl" 85;
	setAttr ".coi" 11.417530380953668;
	setAttr ".dof" yes;
	setAttr ".fs" 1.4;
	setAttr ".fd" 11.5;
select -ne :defaultRenderGlobals;
	setAttr ".imageFilePrefix" -type "string" "<Scene>";
createNode standardSurface -n "postMat";
	setAttr ".bc" -type "float3" 0.85 0.75 0.3 ;
	setAttr ".sr2" 0.4;
createNode shadingEngine -n "postMatSG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode lambert -n "floorMat";
	setAttr ".c" -type "float3" 0.3 0.32 0.35 ;
createNode shadingEngine -n "floorMatSG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode file -n "skyImage";
	setAttr ".ftn" -type "string" "../textures/sky.hdr";
	setAttr ".cs" -type "string" "scene-linear Rec.709-sRGB";
createNode lightLinker -s -n "lightLinker1";
createNode aiOptions -s -n "defaultArnoldRenderOptions";
	setAttr ".AASamples" 4;
createNode aiAOVFilter -s -n "defaultArnoldFilter";
createNode aiAOVDriver -s -n "defaultArnoldDriver";
createNode aiAOVDriver -s -n "defaultArnoldDisplayDriver";
select -ne :lightList1;
	setAttr -s 1 ".l";
createNode script -n "sceneConfigurationScriptNode";
	setAttr ".b" -type "string" "playbackOptions -min 1 -max 150 -ast 1 -aet 240 ";
	setAttr ".st" 6;
select -ne :time1;
	setAttr ".o" 1;
	setAttr ".unw" 1;
relationship "link" ":lightLinker1" ":initialShadingGroup.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "postMatSG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "floorMatSG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" ":initialShadingGroup.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "postMatSG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "floorMatSG.message" ":defaultLightSet.message";
connectAttr "floorMat.oc" "floorMatSG.ss";
connectAttr "postMat.oc" "postMatSG.ss";
connectAttr "skyImage.oc" "skyShape.cl";
connectAttr "floorMat.msg" ":defaultShaderList1.s" -na;
connectAttr "floorMatSG.pa" ":renderPartition.st" -na;
connectAttr "floorShape.iog" "floorMatSG.dsm" -na;
connectAttr "postMat.msg" ":defaultShaderList1.s" -na;
connectAttr "postMatSG.pa" ":renderPartition.st" -na;
connectAttr "postShape0.iog" "postMatSG.dsm" -na;
connectAttr "postShape1.iog" "postMatSG.dsm" -na;
connectAttr "postShape2.iog" "postMatSG.dsm" -na;
connectAttr "postShape3.iog" "postMatSG.dsm" -na;
connectAttr "postShape4.iog" "postMatSG.dsm" -na;
connectAttr "postShape5.iog" "postMatSG.dsm" -na;
connectAttr "postShape6.iog" "postMatSG.dsm" -na;
connectAttr "postShape7.iog" "postMatSG.dsm" -na;
connectAttr "postShape8.iog" "postMatSG.dsm" -na;
connectAttr "skyImage.msg" ":defaultTextureList1.tx" -na;
connectAttr "skyShape.ltd" ":lightList1.l" -na;
// End of depth_of_field.ma
