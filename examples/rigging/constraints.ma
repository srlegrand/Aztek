//Maya ASCII 2024 scene
//Name: constraints.ma
//Codeset: UTF-8
requires maya "2024";
currentUnit -l centimeter -a degree -t film;
fileInfo "application" "aztek";
fileInfo "version" "1.0.0";
createNode transform -s -n "persp";
	setAttr ".v" no;
	setAttr ".t" -type "double3" 0 9 14 ;
	setAttr ".r" -type "double3" -29.74488129694222 0 0 ;
createNode camera -s -n "perspShape" -p "persp";
	setAttr ".coi" 16.1245154965971;
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
createNode transform -n "driver";
	setAttr ".t" -type "double3" -4 2 0 ;
createNode locator -n "driverShape" -p "driver";
	setAttr -k off ".v";
createNode transform -n "pointFollower";
	setAttr ".t" -type "double3" -4 2 0 ;
createNode mesh -n "pointFollowerShape" -p "pointFollower";
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
	setAttr ".vt[0:7]"  -0.4 -0.4 0.4 0.4 -0.4 0.4 0.4 0.4 0.4 -0.4 0.4 0.4 0.4 0.4 -0.4 -0.4 0.4 -0.4 0.4 -0.4 -0.4 -0.4 -0.4 -0.4;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "orientFollower";
	setAttr ".t" -type "double3" -2.5 0.4 -4 ;
createNode mesh -n "orientFollowerShape" -p "orientFollower";
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
	setAttr ".vt[0:7]"  -0.4 -0.4 0.4 0.4 -0.4 0.4 0.4 0.4 0.4 -0.4 0.4 0.4 0.4 0.4 -0.4 -0.4 0.4 -0.4 0.4 -0.4 -0.4 -0.4 -0.4 -0.4;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "parentFollower";
	setAttr ".t" -type "double3" 0 0.3999999999999999 -4 ;
createNode mesh -n "parentFollowerShape" -p "parentFollower";
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
	setAttr ".vt[0:7]"  -0.4 -0.4 0.4 0.4 -0.4 0.4 0.4 0.4 0.4 -0.4 0.4 0.4 0.4 0.4 -0.4 -0.4 0.4 -0.4 0.4 -0.4 -0.4 -0.4 -0.4 -0.4;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "aimFollower";
	setAttr ".t" -type "double3" 2.5 0.4 -4 ;
	setAttr ".r" -type "double3" 101.83996590249052 58.3924977537511 180 ;
createNode mesh -n "aimFollowerShape" -p "aimFollower";
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
	setAttr ".vt[0:7]"  -0.4 -0.4 0.4 0.4 -0.4 0.4 0.4 0.4 0.4 -0.4 0.4 0.4 0.4 0.4 -0.4 -0.4 0.4 -0.4 0.4 -0.4 -0.4 -0.4 -0.4 -0.4;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "scaleFollower";
	setAttr ".t" -type "double3" 5 0.4 -4 ;
createNode mesh -n "scaleFollowerShape" -p "scaleFollower";
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
	setAttr ".vt[0:7]"  -0.4 -0.4 0.4 0.4 -0.4 0.4 0.4 0.4 0.4 -0.4 0.4 0.4 0.4 0.4 -0.4 -0.4 0.4 -0.4 0.4 -0.4 -0.4 -0.4 -0.4 -0.4;
	setAttr -s 12 ".ed";
	setAttr ".ed[0:11]"  0 1 0 1 2 0 2 3 0 3 0 0 2 4 0 4 5 0 5 3 0 4 6 0 6 7 0 7 5 0 6 1 0 0 7 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 -3 4 5 6 mu 0 4 3 2 4 5 
		f 4 -6 7 8 9 mu 0 4 5 4 6 7 
		f 4 -9 10 -1 11 mu 0 4 7 6 8 9 
		f 4 -11 -8 -5 -2 mu 0 4 1 10 11 2 
		f 4 -12 -4 -7 -10 mu 0 4 12 0 3 13 ;
createNode transform -n "hill";
	setAttr ".t" -type "double3" 0 0 4 ;
createNode mesh -n "hillShape" -p "hill";
	setAttr -k off ".v";
	setAttr ".vir" yes;
	setAttr ".vif" yes;
	setAttr ".uvst[0].uvsn" -type "string" "map1";
	setAttr -s 105 ".uvst[0].uvsp";
	setAttr ".uvst[0].uvsp[0:104]" -type "float2" 0 0 0.05 0 0.05 0.25 0 0.25 0.1 0 0.1 0.25 0.15 0 0.15 0.25 0.2 0 0.2 0.25 0.25 0 0.25 0.25 0.3 0 0.3 0.25 0.35 0 0.35 0.25 0.4 0 0.4 0.25 0.45 0 0.45 0.25 0.5 0 0.5 0.25 0.55 0 0.55 0.25 0.6 0 0.6 0.25 0.65 0 0.65 0.25 0.7 0 0.7 0.25 0.75 0 0.75 0.25 0.8 0 0.8 0.25 0.85 0 0.85 0.25 0.9 0 0.9 0.25 0.95 0 0.95 0.25 1 0 1 0.25 0.05 0.5 0 0.5 0.1 0.5 0.15 0.5 0.2 0.5 0.25 0.5 0.3 0.5 0.35 0.5 0.4 0.5 0.45 0.5 0.5 0.5 0.55 0.5 0.6 0.5 0.65 0.5 0.7 0.5 0.75 0.5 0.8 0.5 0.85 0.5 0.9 0.5 0.95 0.5 1 0.5 0.05 0.75 0 0.75 0.1 0.75 0.15 0.75 0.2 0.75 0.25 0.75 0.3 0.75 0.35 0.75 0.4 0.75 0.45 0.75 0.5 0.75 0.55 0.75 0.6 0.75 0.65 0.75 0.7 0.75 0.75 0.75 0.8 0.75 0.85 0.75 0.9 0.75 0.95 0.75 1 0.75 0.05 1 0 1 0.1 1 0.15 1 0.2 1 0.25 1 0.3 1 0.35 1 0.4 1 0.45 1 0.5 1 0.55 1 0.6 1 0.65 1 0.7 1 0.75 1 0.8 1 0.85 1 0.9 1 0.95 1 1 1 ;
	setAttr ".cuvs" -type "string" "map1";
	setAttr ".dcc" -type "string" "Ambient+Diffuse";
	setAttr ".covm[0]"  0 1 1;
	setAttr ".cdvm[0]"  0 1 1;
	setAttr -s 105 ".vt";
	setAttr ".vt[0:104]"  -5 0.4540814971847569 2 -4.5 0.26551226597691147 2 -4 0.03502448605654805 2 -3.5 -0.2009928900935428 2 -3 -0.4052779083306904 2 -2.5 -0.545578456095409 2 -2 -0.5997441618249031 2 -1.5000000000000002 -0.5592234515803358 2 -0.9999999999999998 -0.4304136545397136 2 -0.4999999999999999 -0.23365100538519024 2 0 0 2 0.5000000000000004 0.23365100538519046 2 0.9999999999999998 0.4304136545397136 2 1.5000000000000002 0.5592234515803358 2 1.9999999999999996 0.5997441618249031 2 2.5 0.545578456095409 2 3.0000000000000004 0.4052779083306904 2 3.5 0.2009928900935428 2 4 -0.03502448605654805 2 4.5 -0.26551226597691147 2 5 -0.4540814971847569 2 -5 0.4540814971847569 1 -4.5 0.26551226597691147 1 -4 0.03502448605654805 1 -3.5 -0.2009928900935428 1 -3 -0.4052779083306904 1 -2.5 -0.545578456095409 1 -2 -0.5997441618249031 1 -1.5000000000000002 -0.5592234515803358 1 -0.9999999999999998 -0.4304136545397136 1 -0.4999999999999999 -0.23365100538519024 1 0 0 1 0.5000000000000004 0.23365100538519046 1 0.9999999999999998 0.4304136545397136 1 1.5000000000000002 0.5592234515803358 1 1.9999999999999996 0.5997441618249031 1 2.5 0.545578456095409 1 3.0000000000000004 0.4052779083306904 1 3.5 0.2009928900935428 1 4 -0.03502448605654805 1 4.5 -0.26551226597691147 1 5 -0.4540814971847569 1 -5 0.4540814971847569 0 -4.5 0.26551226597691147 0 -4 0.03502448605654805 0 -3.5 -0.2009928900935428 0 -3 -0.4052779083306904 0 -2.5 -0.545578456095409 0 -2 -0.5997441618249031 0 -1.5000000000000002 -0.5592234515803358 0 -0.9999999999999998 -0.4304136545397136 0 -0.4999999999999999 -0.23365100538519024 0 0 0 0 0.5000000000000004 0.23365100538519046 0 0.9999999999999998 0.4304136545397136 0 1.5000000000000002 0.5592234515803358 0 1.9999999999999996 0.5997441618249031 0 2.5 0.545578456095409 0 3.0000000000000004 0.4052779083306904 0 3.5 0.2009928900935428 0 4 -0.03502448605654805 0 4.5 -0.26551226597691147 0 5 -0.4540814971847569 0 -5 0.4540814971847569 -1 -4.5 0.26551226597691147 -1 -4 0.03502448605654805 -1 -3.5 -0.2009928900935428 -1 -3 -0.4052779083306904 -1 -2.5 -0.545578456095409 -1 -2 -0.5997441618249031 -1 -1.5000000000000002 -0.5592234515803358 -1 -0.9999999999999998 -0.4304136545397136 -1 -0.4999999999999999 -0.23365100538519024 -1 0 0 -1 0.5000000000000004 0.23365100538519046 -1 0.9999999999999998 0.4304136545397136 -1 1.5000000000000002 0.5592234515803358 -1 1.9999999999999996 0.5997441618249031 -1 2.5 0.545578456095409 -1 3.0000000000000004 0.4052779083306904 -1 3.5 0.2009928900935428 -1 4 -0.03502448605654805 -1 4.5 -0.26551226597691147 -1 5 -0.4540814971847569 -1 -5 0.4540814971847569 -2 -4.5 0.26551226597691147 -2 -4 0.03502448605654805 -2 -3.5 -0.2009928900935428 -2 -3 -0.4052779083306904 -2 -2.5 -0.545578456095409 -2 -2 -0.5997441618249031 -2 -1.5000000000000002 -0.5592234515803358 -2 -0.9999999999999998 -0.4304136545397136 -2 -0.4999999999999999 -0.23365100538519024 -2 0 0 -2 0.5000000000000004 0.23365100538519046 -2 0.9999999999999998 0.4304136545397136 -2 1.5000000000000002 0.5592234515803358 -2 1.9999999999999996 0.5997441618249031 -2 2.5 0.545578456095409 -2 3.0000000000000004 0.4052779083306904 -2 3.5 0.2009928900935428 -2 4 -0.03502448605654805 -2 4.5 -0.26551226597691147 -2 5 -0.4540814971847569 -2;
	setAttr -s 184 ".ed";
	setAttr ".ed[0:165]"  0 1 1 1 22 1 22 21 1 21 0 1 1 2 1 2 23 1 23 22 1 2 3 1 3 24 1 24 23 1 3 4 1 4 25 1 25 24 1 4 5 1 5 26 1 26 25 1 5 6 1 6 27 1 27 26 1 6 7 1 7 28 1 28 27 1 7 8 1 8 29 1 29 28 1 8 9 1 9 30 1 30 29 1 9 10 1 10 31 1 31 30 1 10 11 1 11 32 1 32 31 1 11 12 1 12 33 1 33 32 1 12 13 1 13 34 1 34 33 1 13 14 1 14 35 1 35 34 1 14 15 1 15 36 1 36 35 1 15 16 1 16 37 1 37 36 1 16 17 1 17 38 1 38 37 1 17 18 1 18 39 1 39 38 1 18 19 1 19 40 1 40 39 1 19 20 1 20 41 1 41 40 1 22 43 1 43 42 1 42 21 1 23 44 1 44 43 1 24 45 1 45 44 1 25 46 1 46 45 1 26 47 1 47 46 1 27 48 1 48 47 1 28 49 1 49 48 1 29 50 1 50 49 1 30 51 1 51 50 1 31 52 1 52 51 1 32 53 1 53 52 1 33 54 1 54 53 1 34 55 1 55 54 1 35 56 1 56 55 1 36 57 1 57 56 1 37 58 1 58 57 1 38 59 1 59 58 1 39 60 1 60 59 1 40 61 1 61 60 1 41 62 1 62 61 1 43 64 1 64 63 1 63 42 1 44 65 1 65 64 1 45 66 1 66 65 1 46 67 1 67 66 1 47 68 1 68 67 1 48 69 1 69 68 1 49 70 1 70 69 1 50 71 1 71 70 1 51 72 1 72 71 1 52 73 1 73 72 1 53 74 1 74 73 1 54 75 1 75 74 1 55 76 1 76 75 1 56 77 1 77 76 1 57 78 1 78 77 1 58 79 1 79 78 1 59 80 1 80 79 1 60 81 1 81 80 1 61 82 1 82 81 1 62 83 1 83 82 1 64 85 1 85 84 1 84 63 1 65 86 1 86 85 1 66 87 1 87 86 1 67 88 1 88 87 1 68 89 1 89 88 1 69 90 1 90 89 1 70 91 1 91 90 1 71 92 1 92 91 1 72 93 1 93 92 1 73 94 1 94 93 1 74 95 1 95 94 1;
	setAttr ".ed[166:183]"  75 96 1 96 95 1 76 97 1 97 96 1 77 98 1 98 97 1 78 99 1 99 98 1 79 100 1 100 99 1 80 101 1 101 100 1 81 102 1 102 101 1 82 103 1 103 102 1 83 104 1 104 103 1;
	setAttr -s 80 -ch 320 ".fc[0:79]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 4 5 6 -2 mu 0 4 1 4 5 2 
		f 4 7 8 9 -6 mu 0 4 4 6 7 5 
		f 4 10 11 12 -9 mu 0 4 6 8 9 7 
		f 4 13 14 15 -12 mu 0 4 8 10 11 9 
		f 4 16 17 18 -15 mu 0 4 10 12 13 11 
		f 4 19 20 21 -18 mu 0 4 12 14 15 13 
		f 4 22 23 24 -21 mu 0 4 14 16 17 15 
		f 4 25 26 27 -24 mu 0 4 16 18 19 17 
		f 4 28 29 30 -27 mu 0 4 18 20 21 19 
		f 4 31 32 33 -30 mu 0 4 20 22 23 21 
		f 4 34 35 36 -33 mu 0 4 22 24 25 23 
		f 4 37 38 39 -36 mu 0 4 24 26 27 25 
		f 4 40 41 42 -39 mu 0 4 26 28 29 27 
		f 4 43 44 45 -42 mu 0 4 28 30 31 29 
		f 4 46 47 48 -45 mu 0 4 30 32 33 31 
		f 4 49 50 51 -48 mu 0 4 32 34 35 33 
		f 4 52 53 54 -51 mu 0 4 34 36 37 35 
		f 4 55 56 57 -54 mu 0 4 36 38 39 37 
		f 4 58 59 60 -57 mu 0 4 38 40 41 39 
		f 4 -3 61 62 63 mu 0 4 3 2 42 43 
		f 4 -7 64 65 -62 mu 0 4 2 5 44 42 
		f 4 -10 66 67 -65 mu 0 4 5 7 45 44 
		f 4 -13 68 69 -67 mu 0 4 7 9 46 45 
		f 4 -16 70 71 -69 mu 0 4 9 11 47 46 
		f 4 -19 72 73 -71 mu 0 4 11 13 48 47 
		f 4 -22 74 75 -73 mu 0 4 13 15 49 48 
		f 4 -25 76 77 -75 mu 0 4 15 17 50 49 
		f 4 -28 78 79 -77 mu 0 4 17 19 51 50 
		f 4 -31 80 81 -79 mu 0 4 19 21 52 51 
		f 4 -34 82 83 -81 mu 0 4 21 23 53 52 
		f 4 -37 84 85 -83 mu 0 4 23 25 54 53 
		f 4 -40 86 87 -85 mu 0 4 25 27 55 54 
		f 4 -43 88 89 -87 mu 0 4 27 29 56 55 
		f 4 -46 90 91 -89 mu 0 4 29 31 57 56 
		f 4 -49 92 93 -91 mu 0 4 31 33 58 57 
		f 4 -52 94 95 -93 mu 0 4 33 35 59 58 
		f 4 -55 96 97 -95 mu 0 4 35 37 60 59 
		f 4 -58 98 99 -97 mu 0 4 37 39 61 60 
		f 4 -61 100 101 -99 mu 0 4 39 41 62 61 
		f 4 -63 102 103 104 mu 0 4 43 42 63 64 
		f 4 -66 105 106 -103 mu 0 4 42 44 65 63 
		f 4 -68 107 108 -106 mu 0 4 44 45 66 65 
		f 4 -70 109 110 -108 mu 0 4 45 46 67 66 
		f 4 -72 111 112 -110 mu 0 4 46 47 68 67 
		f 4 -74 113 114 -112 mu 0 4 47 48 69 68 
		f 4 -76 115 116 -114 mu 0 4 48 49 70 69 
		f 4 -78 117 118 -116 mu 0 4 49 50 71 70 
		f 4 -80 119 120 -118 mu 0 4 50 51 72 71 
		f 4 -82 121 122 -120 mu 0 4 51 52 73 72 
		f 4 -84 123 124 -122 mu 0 4 52 53 74 73 
		f 4 -86 125 126 -124 mu 0 4 53 54 75 74 
		f 4 -88 127 128 -126 mu 0 4 54 55 76 75 
		f 4 -90 129 130 -128 mu 0 4 55 56 77 76 
		f 4 -92 131 132 -130 mu 0 4 56 57 78 77 
		f 4 -94 133 134 -132 mu 0 4 57 58 79 78 
		f 4 -96 135 136 -134 mu 0 4 58 59 80 79 
		f 4 -98 137 138 -136 mu 0 4 59 60 81 80 
		f 4 -100 139 140 -138 mu 0 4 60 61 82 81 
		f 4 -102 141 142 -140 mu 0 4 61 62 83 82 
		f 4 -104 143 144 145 mu 0 4 64 63 84 85 
		f 4 -107 146 147 -144 mu 0 4 63 65 86 84 
		f 4 -109 148 149 -147 mu 0 4 65 66 87 86 
		f 4 -111 150 151 -149 mu 0 4 66 67 88 87 
		f 4 -113 152 153 -151 mu 0 4 67 68 89 88 
		f 4 -115 154 155 -153 mu 0 4 68 69 90 89 
		f 4 -117 156 157 -155 mu 0 4 69 70 91 90 
		f 4 -119 158 159 -157 mu 0 4 70 71 92 91 
		f 4 -121 160 161 -159 mu 0 4 71 72 93 92 
		f 4 -123 162 163 -161 mu 0 4 72 73 94 93 
		f 4 -125 164 165 -163 mu 0 4 73 74 95 94 
		f 4 -127 166 167 -165 mu 0 4 74 75 96 95 
		f 4 -129 168 169 -167 mu 0 4 75 76 97 96 
		f 4 -131 170 171 -169 mu 0 4 76 77 98 97 
		f 4 -133 172 173 -171 mu 0 4 77 78 99 98 
		f 4 -135 174 175 -173 mu 0 4 78 79 100 99 
		f 4 -137 176 177 -175 mu 0 4 79 80 101 100 
		f 4 -139 178 179 -177 mu 0 4 80 81 102 101 
		f 4 -141 180 181 -179 mu 0 4 81 82 103 102 
		f 4 -143 182 183 -181 mu 0 4 82 83 104 103 ;
createNode transform -n "rider";
	setAttr ".t" -type "double3" -3.986479761726878 0.028642463732255402 2 ;
	setAttr ".r" -type "double3" 180 0 154.99244977553232 ;
createNode mesh -n "riderShape" -p "rider";
	setAttr -k off ".v";
	setAttr ".vir" yes;
	setAttr ".vif" yes;
	setAttr ".uvst[0].uvsn" -type "string" "map1";
	setAttr -s 116 ".uvst[0].uvsp";
	setAttr ".uvst[0].uvsp[0:115]" -type "float2" 0.41389 0.2964013 0.37937778 0.25 0.41389 0.20359871 0.44047618 0.25 0.33965206 0.20402384 0.38021082 0.15950209 0.29597616 0.16034792 0.3404979 0.1197892 0.25 0.1206222 0.2964013 0.086109996 0.20359871 0.086109996 0.25 0.05952382 0.38021082 0.3404979 0.33965206 0.29597616 0.2957594 0.25 0.25 0.20424059 0.20402384 0.16034792 0.15950209 0.1197892 0.3404979 0.38021082 0.29597616 0.33965206 0.25 0.2957594 0.20424059 0.25 0.16034792 0.20402384 0.1197892 0.15950209 0.2964013 0.41389 0.25 0.37937778 0.20402384 0.33965206 0.16034792 0.29597616 0.1206222 0.25 0.086109996 0.20359871 0.25 0.44047618 0.20359871 0.41389 0.15950209 0.38021082 0.1197892 0.3404979 0.086109996 0.2964013 0.05952382 0.25 0.47825354 0.17583592 0.49 0.25 0.44416407 0.10893156 0.39106846 0.055835932 0.3241641 0.021746457 0.25 0.010000005 0.17583592 0.021746457 0.10893156 0.055835932 0.055835932 0.10893156 0.021746457 0.17583592 0.010000005 0.25 0.021746457 0.3241641 0.055835932 0.39106846 0.10893156 0.44416407 0.17583592 0.47825354 0.25 0.49 0.3241641 0.47825354 0.39106846 0.44416407 0.44416407 0.39106846 0.47825354 0.3241641 0 0.5 0.05 0.5 0.025 1 0.05 0.5 0.1 0.5 0.075 1 0.1 0.5 0.15 0.5 0.125 1 0.15 0.5 0.2 0.5 0.17500001 1 0.2 0.5 0.25 0.5 0.225 1 0.25 0.5 0.3 0.5 0.275 1 0.3 0.5 0.35 0.5 0.325 1 0.35 0.5 0.4 0.5 0.375 1 0.4 0.5 0.45 0.5 0.425 1 0.45 0.5 0.5 0.5 0.475 1 0.5 0.5 0.55 0.5 0.525 1 0.55 0.5 0.6 0.5 0.57500005 1 0.6 0.5 0.65 0.5 0.625 1 0.65 0.5 0.7 0.5 0.67499995 1 0.7 0.5 0.75 0.5 0.725 1 0.75 0.5 0.8 0.5 0.775 1 0.8 0.5 0.85 0.5 0.82500005 1 0.85 0.5 0.9 0.5 0.875 1 0.9 0.5 0.95 0.5 0.92499995 1 0.95 0.5 1 0.5 0.975 1 ;
	setAttr ".cuvs" -type "string" "map1";
	setAttr ".dcc" -type "string" "Ambient+Diffuse";
	setAttr ".covm[0]"  0 1 1;
	setAttr ".cdvm[0]"  0 1 1;
	setAttr -s 57 ".vt";
	setAttr ".vt[0:56]"  0.3 -0.4 0 0.285316954888546 -0.4 -0.09270509831248422 0.2427050983124842 -0.4 -0.17633557568774194 0.17633557568774194 -0.4 -0.2427050983124842 0.09270509831248423 -0.4 -0.285316954888546 0.000000000000000018369701987210297 -0.4 -0.3 -0.0927050983124842 -0.4 -0.2853169548885461 -0.1763355756877419 -0.4 -0.2427050983124842 -0.24270509831248419 -0.4 -0.17633557568774197 -0.285316954888546 -0.4 -0.09270509831248425 -0.3 -0.4 -0.000000000000000036739403974420595 -0.28531695488854614 -0.4 0.09270509831248407 -0.24270509831248427 -0.4 0.1763355756877419 -0.17633557568774197 -0.4 0.24270509831248419 -0.09270509831248426 -0.4 0.285316954888546 -0.00000000000000005510910596163089 -0.4 0.3 0.09270509831248416 -0.4 0.2853169548885461 0.17633557568774186 -0.4 0.24270509831248427 0.24270509831248419 -0.4 0.176335575687742 0.285316954888546 -0.4 0.09270509831248429 0 0.4 0 0.23809523809523808 -0.4 0 0.20486252536144317 -0.4000000000000001 -0.05800161851189975 0.1627635232738045 -0.4 -0.11312240498023554 0.11312240498023554 -0.4 -0.1627635232738045 0.05800161851189975 -0.4 -0.20486252536144317 0.000000000000000014579128561278015 -0.4 -0.23809523809523808 -0.058001618511899736 -0.4000000000000001 -0.20486252536144317 -0.11312240498023551 -0.4 -0.1627635232738045 -0.1627635232738045 -0.4 -0.11312240498023554 -0.20486252536144317 -0.4 -0.058001618511899763 -0.23809523809523808 -0.4 -0.00000000000000002915825712255603 -0.2048625253614432 -0.4000000000000001 0.05800161851189969 -0.16276352327380456 -0.4 0.11312240498023551 -0.11312240498023556 -0.4 0.1627635232738045 -0.05800161851189978 -0.4 0.20486252536144317 -0.00000000000000004373738568383404 -0.4 0.23809523809523808 0.05800161851189971 -0.4000000000000001 0.20486252536144317 0.1131224049802355 -0.4 0.16276352327380456 0.1627635232738045 -0.4 0.11312240498023557 0.20486252536144317 -0.4 0.05800161851189979 0.16172225031640633 -0.4000000000000002 0.00000000000000003586079980443838 0.11206511019700725 -0.39999999999999997 -0.057470210747817316 0.057470210747817316 -0.4000000000000002 -0.11206511019700725 0 -0.39999999999999997 -0.16172225031640633 0.11206511019700727 -0.40000000000000013 0.05747021074781737 0.05719925632532938 -0.4000000000000001 0.00000000000000001854310894742342 -0.000000000000000013877787807814457 -0.4 -0.05719925632532936 -0.05747021074781733 -0.4 -0.11206511019700725 0.057470210747817316 -0.4 0.11206511019700734 -0.000000000000000013877787807814457 -0.4 0.0571992563253294 -0.0571992563253294 -0.4 0.000000000000000024374760371934625 -0.1120651101970073 -0.4 -0.05747021074781736 -0.000000000000000027755575615628914 -0.4000000000000001 0.16172225031640633 -0.057470210747817344 -0.40000000000000013 0.11206511019700725 -0.11206511019700732 -0.4 0.05747021074781733 -0.16172225031640633 -0.40000000000000013 -0.000000000000000043788760576729195;
	setAttr -s 120 ".ed";
	setAttr ".ed[0:119]"  40 41 1 41 22 1 22 21 1 21 40 1 41 42 1 42 23 1 23 22 1 42 43 1 43 24 1 24 23 1 43 44 1 44 25 1 25 24 1 44 27 1 27 26 1 26 25 1 39 45 1 45 41 1 40 39 1 45 46 1 46 42 1 46 47 1 47 43 1 47 48 1 48 44 1 48 28 1 28 27 1 38 49 1 49 45 1 39 38 1 49 50 1 50 46 1 50 51 1 51 47 1 51 52 1 52 48 1 52 29 1 29 28 1 37 53 1 53 49 1 38 37 1 53 54 1 54 50 1 54 55 1 55 51 1 55 56 1 56 52 1 56 30 1 30 29 1 36 35 1 35 53 1 37 36 1 35 34 1 34 54 1 34 33 1 33 55 1 33 32 1 32 56 1 32 31 1 31 30 1 22 1 1 1 0 0 0 21 1 23 2 1 2 1 0 24 3 1 3 2 0 25 4 1 4 3 0 26 5 1 5 4 0 27 6 1 6 5 0 28 7 1 7 6 0 29 8 1 8 7 0 30 9 1 9 8 0 31 10 1 10 9 0 32 11 1 11 10 0 33 12 1 12 11 0 34 13 1 13 12 0 35 14 1 14 13 0 36 15 1 15 14 0 37 16 1 16 15 0 38 17 1 17 16 0 39 18 1 18 17 0 40 19 1 19 18 0 0 19 0 1 20 1 20 0 1 2 20 1 3 20 1 4 20 1 5 20 1 6 20 1 7 20 1 8 20 1 9 20 1 10 20 1 11 20 1 12 20 1 13 20 1 14 20 1 15 20 1 16 20 1 17 20 1 18 20 1 19 20 1;
	setAttr -s 65 -ch 240 ".fc[0:64]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 4 5 6 -2 mu 0 4 1 4 5 2 
		f 4 7 8 9 -6 mu 0 4 4 6 7 5 
		f 4 10 11 12 -9 mu 0 4 6 8 9 7 
		f 4 13 14 15 -12 mu 0 4 8 10 11 9 
		f 4 16 17 -1 18 mu 0 4 12 13 1 0 
		f 4 19 20 -5 -18 mu 0 4 13 14 4 1 
		f 4 21 22 -8 -21 mu 0 4 14 15 6 4 
		f 4 23 24 -11 -23 mu 0 4 15 16 8 6 
		f 4 25 26 -14 -25 mu 0 4 16 17 10 8 
		f 4 27 28 -17 29 mu 0 4 18 19 13 12 
		f 4 30 31 -20 -29 mu 0 4 19 20 14 13 
		f 4 32 33 -22 -32 mu 0 4 20 21 15 14 
		f 4 34 35 -24 -34 mu 0 4 21 22 16 15 
		f 4 36 37 -26 -36 mu 0 4 22 23 17 16 
		f 4 38 39 -28 40 mu 0 4 24 25 19 18 
		f 4 41 42 -31 -40 mu 0 4 25 26 20 19 
		f 4 43 44 -33 -43 mu 0 4 26 27 21 20 
		f 4 45 46 -35 -45 mu 0 4 27 28 22 21 
		f 4 47 48 -37 -47 mu 0 4 28 29 23 22 
		f 4 49 50 -39 51 mu 0 4 30 31 25 24 
		f 4 52 53 -42 -51 mu 0 4 31 32 26 25 
		f 4 54 55 -44 -54 mu 0 4 32 33 27 26 
		f 4 56 57 -46 -56 mu 0 4 33 34 28 27 
		f 4 58 59 -48 -58 mu 0 4 34 35 29 28 
		f 4 -3 60 61 62 mu 0 4 3 2 36 37 
		f 4 -7 63 64 -61 mu 0 4 2 5 38 36 
		f 4 -10 65 66 -64 mu 0 4 5 7 39 38 
		f 4 -13 67 68 -66 mu 0 4 7 9 40 39 
		f 4 -16 69 70 -68 mu 0 4 9 11 41 40 
		f 4 -15 71 72 -70 mu 0 4 11 10 42 41 
		f 4 -27 73 74 -72 mu 0 4 10 17 43 42 
		f 4 -38 75 76 -74 mu 0 4 17 23 44 43 
		f 4 -49 77 78 -76 mu 0 4 23 29 45 44 
		f 4 -60 79 80 -78 mu 0 4 29 35 46 45 
		f 4 -59 81 82 -80 mu 0 4 35 34 47 46 
		f 4 -57 83 84 -82 mu 0 4 34 33 48 47 
		f 4 -55 85 86 -84 mu 0 4 33 32 49 48 
		f 4 -53 87 88 -86 mu 0 4 32 31 50 49 
		f 4 -50 89 90 -88 mu 0 4 31 30 51 50 
		f 4 -52 91 92 -90 mu 0 4 30 24 52 51 
		f 4 -41 93 94 -92 mu 0 4 24 18 53 52 
		f 4 -30 95 96 -94 mu 0 4 18 12 54 53 
		f 4 -19 97 98 -96 mu 0 4 12 0 55 54 
		f 4 -4 -63 99 -98 mu 0 4 0 3 37 55 
		f 3 -62 100 101 mu 0 3 56 57 58 
		f 3 -65 102 -101 mu 0 3 59 60 61 
		f 3 -67 103 -103 mu 0 3 62 63 64 
		f 3 -69 104 -104 mu 0 3 65 66 67 
		f 3 -71 105 -105 mu 0 3 68 69 70 
		f 3 -73 106 -106 mu 0 3 71 72 73 
		f 3 -75 107 -107 mu 0 3 74 75 76 
		f 3 -77 108 -108 mu 0 3 77 78 79 
		f 3 -79 109 -109 mu 0 3 80 81 82 
		f 3 -81 110 -110 mu 0 3 83 84 85 
		f 3 -83 111 -111 mu 0 3 86 87 88 
		f 3 -85 112 -112 mu 0 3 89 90 91 
		f 3 -87 113 -113 mu 0 3 92 93 94 
		f 3 -89 114 -114 mu 0 3 95 96 97 
		f 3 -91 115 -115 mu 0 3 98 99 100 
		f 3 -93 116 -116 mu 0 3 101 102 103 
		f 3 -95 117 -117 mu 0 3 104 105 106 
		f 3 -97 118 -118 mu 0 3 107 108 109 
		f 3 -99 119 -119 mu 0 3 110 111 112 
		f 3 -100 -102 -120 mu 0 3 113 114 115 ;
createNode transform -n "keyLight";
	setAttr ".r" -type "double3" -45 35 0 ;
createNode directionalLight -n "keyLightShape" -p "keyLight";
	setAttr -k off ".v";
createNode transform -n "fillLight";
createNode ambientLight -n "fillLightShape" -p "fillLight";
	setAttr -k off ".v";
	setAttr ".in" 0.25;
createNode animCurveTL -n "driver_translateX";
	setAttr ".tan" 18;
	setAttr ".wgt" no;
	setAttr -s 3 ".ktv[0:2]"  1 -4 60 4 120 -4;
createNode animCurveTL -n "driver_translateY";
	setAttr ".tan" 18;
	setAttr ".wgt" no;
	setAttr -s 5 ".ktv[0:4]"  1 2 30 4 60 2 90 4 120 2;
createNode animCurveTA -n "driver_rotateZ";
	setAttr ".tan" 18;
	setAttr ".wgt" no;
	setAttr -s 2 ".ktv[0:1]"  1 0 120 360;
createNode animCurveTU -n "driver_scaleX";
	setAttr ".tan" 18;
	setAttr ".wgt" no;
	setAttr -s 3 ".ktv[0:2]"  1 1 60 2 120 1;
createNode blinn -n "followerMat";
	setAttr ".c" -type "float3" 0.9 0.6 0.2 ;
createNode shadingEngine -n "followerMatSG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode lambert -n "hillMat";
	setAttr ".c" -type "float3" 0.3 0.6 0.3 ;
createNode shadingEngine -n "hillMatSG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode pointConstraint -n "pointFollower_pointConstraint1" -p "pointFollower";
	setAttr -s 1 ".tg";
	addAttr -dcb 0 -ci true -k true -sn "w0" -ln "driverW0" -dv 1 -min 0 -at "double";
	setAttr ".rst" -type "double3" -5 0.4 -4 ;
	setAttr ".rsrr" -type "double3" 0 0 0 ;
	setAttr ".erp" yes;
createNode orientConstraint -n "orientFollower_orientConstraint1" -p "orientFollower";
	setAttr -s 1 ".tg";
	addAttr -dcb 0 -ci true -k true -sn "w0" -ln "driverW0" -dv 1 -min 0 -at "double";
	setAttr ".rst" -type "double3" -2.5 0.4 -4 ;
	setAttr ".rsrr" -type "double3" 0 0 0 ;
	setAttr ".erp" yes;
createNode parentConstraint -n "parentFollower_parentConstraint1" -p "parentFollower";
	setAttr -s 1 ".tg";
	addAttr -dcb 0 -ci true -k true -sn "w0" -ln "driverW0" -dv 1 -min 0 -at "double";
	setAttr ".tg[0].tot" -type "double3" 4 -1.6 -4 ;
	setAttr ".rst" -type "double3" 0 0.4 -4 ;
	setAttr ".rsrr" -type "double3" 0 0 0 ;
	setAttr ".erp" yes;
createNode aimConstraint -n "aimFollower_aimConstraint1" -p "aimFollower";
	setAttr -s 1 ".tg";
	addAttr -dcb 0 -ci true -k true -sn "w0" -ln "driverW0" -dv 1 -min 0 -at "double";
	setAttr ".a" -type "double3" 0 1 0 ;
	setAttr ".u" -type "double3" 0 0 1 ;
	setAttr ".wu" -type "double3" 0 1 0 ;
	setAttr ".wut" 3;
	setAttr ".rst" -type "double3" 2.5 0.4 -4 ;
	setAttr ".rsrr" -type "double3" 0 0 0 ;
	setAttr ".erp" yes;
createNode scaleConstraint -n "scaleFollower_scaleConstraint1" -p "scaleFollower";
	setAttr -s 1 ".tg";
	addAttr -dcb 0 -ci true -k true -sn "w0" -ln "driverW0" -dv 1 -min 0 -at "double";
	setAttr ".rst" -type "double3" 5 0.4 -4 ;
	setAttr ".rsrr" -type "double3" 0 0 0 ;
	setAttr ".erp" yes;
createNode geometryConstraint -n "rider_geometryConstraint1" -p "rider";
	setAttr -s 1 ".tg";
	addAttr -dcb 0 -ci true -k true -sn "w0" -ln "hillW0" -dv 1 -min 0 -at "double";
	setAttr ".rst" -type "double3" -4 0 0 ;
	setAttr ".rsrr" -type "double3" 0 0 0 ;
	setAttr ".erp" yes;
createNode normalConstraint -n "rider_normalConstraint1" -p "rider";
	setAttr -s 1 ".tg";
	addAttr -dcb 0 -ci true -k true -sn "w0" -ln "hillW0" -dv 1 -min 0 -at "double";
	setAttr ".a" -type "double3" 0 1 0 ;
	setAttr ".u" -type "double3" 1 0 0 ;
	setAttr ".wu" -type "double3" 0 1 0 ;
	setAttr ".wut" 3;
	setAttr ".rst" -type "double3" -3.986479761726878 0.028642463732255402 2 ;
	setAttr ".rsrr" -type "double3" 0 0 0 ;
	setAttr ".erp" yes;
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
relationship "link" ":lightLinker1" "followerMatSG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "hillMatSG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" ":initialShadingGroup.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "followerMatSG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "hillMatSG.message" ":defaultLightSet.message";
connectAttr "aimFollower.pim" "aimFollower|aimFollower_aimConstraint1.cpim";
connectAttr "aimFollower.ro" "aimFollower|aimFollower_aimConstraint1.cro";
connectAttr "aimFollower.rp" "aimFollower|aimFollower_aimConstraint1.crp";
connectAttr "aimFollower.rpt" "aimFollower|aimFollower_aimConstraint1.crt";
connectAttr "aimFollower|aimFollower_aimConstraint1.crx" "aimFollower.rx";
connectAttr "aimFollower|aimFollower_aimConstraint1.cry" "aimFollower.ry";
connectAttr "aimFollower|aimFollower_aimConstraint1.crz" "aimFollower.rz";
connectAttr "aimFollower|aimFollower_aimConstraint1.w0" "aimFollower|aimFollower_aimConstraint1.tg[0].tw";
connectAttr "driver.pm" "aimFollower|aimFollower_aimConstraint1.tg[0].tpm";
connectAttr "driver.pm" "orientFollower|orientFollower_orientConstraint1.tg[0].tpm";
connectAttr "driver.pm" "parentFollower|parentFollower_parentConstraint1.tg[0].tpm";
connectAttr "driver.pm" "pointFollower|pointFollower_pointConstraint1.tg[0].tpm";
connectAttr "driver.pm" "scaleFollower|scaleFollower_scaleConstraint1.tg[0].tpm";
connectAttr "driver.r" "orientFollower|orientFollower_orientConstraint1.tg[0].tr";
connectAttr "driver.r" "parentFollower|parentFollower_parentConstraint1.tg[0].tr";
connectAttr "driver.ro" "orientFollower|orientFollower_orientConstraint1.tg[0].tro";
connectAttr "driver.ro" "parentFollower|parentFollower_parentConstraint1.tg[0].tro";
connectAttr "driver.rp" "aimFollower|aimFollower_aimConstraint1.tg[0].trp";
connectAttr "driver.rp" "parentFollower|parentFollower_parentConstraint1.tg[0].trp";
connectAttr "driver.rp" "pointFollower|pointFollower_pointConstraint1.tg[0].trp";
connectAttr "driver.rpt" "aimFollower|aimFollower_aimConstraint1.tg[0].trt";
connectAttr "driver.rpt" "parentFollower|parentFollower_parentConstraint1.tg[0].trt";
connectAttr "driver.rpt" "pointFollower|pointFollower_pointConstraint1.tg[0].trt";
connectAttr "driver.s" "parentFollower|parentFollower_parentConstraint1.tg[0].ts";
connectAttr "driver.s" "scaleFollower|scaleFollower_scaleConstraint1.tg[0].ts";
connectAttr "driver.t" "aimFollower|aimFollower_aimConstraint1.tg[0].tt";
connectAttr "driver.t" "parentFollower|parentFollower_parentConstraint1.tg[0].tt";
connectAttr "driver.t" "pointFollower|pointFollower_pointConstraint1.tg[0].tt";
connectAttr "driver_rotateZ.o" "driver.rz";
connectAttr "driver_scaleX.o" "driver.sx";
connectAttr "driver_translateX.o" "driver.tx";
connectAttr "driver_translateY.o" "driver.ty";
connectAttr "followerMat.oc" "followerMatSG.ss";
connectAttr "hill.pm" "rider|rider_geometryConstraint1.tg[0].tpm";
connectAttr "hill.pm" "rider|rider_normalConstraint1.tg[0].tpm";
connectAttr "hill.s" "rider|rider_geometryConstraint1.tg[0].ts";
connectAttr "hill.s" "rider|rider_normalConstraint1.tg[0].ts";
connectAttr "hillMat.oc" "hillMatSG.ss";
connectAttr "orientFollower.pim" "orientFollower|orientFollower_orientConstraint1.cpim";
connectAttr "orientFollower.ro" "orientFollower|orientFollower_orientConstraint1.cro";
connectAttr "orientFollower|orientFollower_orientConstraint1.crx" "orientFollower.rx";
connectAttr "orientFollower|orientFollower_orientConstraint1.cry" "orientFollower.ry";
connectAttr "orientFollower|orientFollower_orientConstraint1.crz" "orientFollower.rz";
connectAttr "orientFollower|orientFollower_orientConstraint1.w0" "orientFollower|orientFollower_orientConstraint1.tg[0].tw";
connectAttr "parentFollower.pim" "parentFollower|parentFollower_parentConstraint1.cpim";
connectAttr "parentFollower.ro" "parentFollower|parentFollower_parentConstraint1.cro";
connectAttr "parentFollower.rp" "parentFollower|parentFollower_parentConstraint1.crp";
connectAttr "parentFollower.rpt" "parentFollower|parentFollower_parentConstraint1.crt";
connectAttr "parentFollower|parentFollower_parentConstraint1.crx" "parentFollower.rx";
connectAttr "parentFollower|parentFollower_parentConstraint1.cry" "parentFollower.ry";
connectAttr "parentFollower|parentFollower_parentConstraint1.crz" "parentFollower.rz";
connectAttr "parentFollower|parentFollower_parentConstraint1.ctx" "parentFollower.tx";
connectAttr "parentFollower|parentFollower_parentConstraint1.cty" "parentFollower.ty";
connectAttr "parentFollower|parentFollower_parentConstraint1.ctz" "parentFollower.tz";
connectAttr "parentFollower|parentFollower_parentConstraint1.w0" "parentFollower|parentFollower_parentConstraint1.tg[0].tw";
connectAttr "pointFollower.pim" "pointFollower|pointFollower_pointConstraint1.cpim";
connectAttr "pointFollower.ro" "pointFollower|pointFollower_pointConstraint1.cro";
connectAttr "pointFollower.rp" "pointFollower|pointFollower_pointConstraint1.crp";
connectAttr "pointFollower.rpt" "pointFollower|pointFollower_pointConstraint1.crt";
connectAttr "pointFollower|pointFollower_pointConstraint1.ctx" "pointFollower.tx";
connectAttr "pointFollower|pointFollower_pointConstraint1.cty" "pointFollower.ty";
connectAttr "pointFollower|pointFollower_pointConstraint1.ctz" "pointFollower.tz";
connectAttr "pointFollower|pointFollower_pointConstraint1.w0" "pointFollower|pointFollower_pointConstraint1.tg[0].tw";
connectAttr "rider.pim" "rider|rider_geometryConstraint1.cpim";
connectAttr "rider.pim" "rider|rider_normalConstraint1.cpim";
connectAttr "rider.ro" "rider|rider_geometryConstraint1.cro";
connectAttr "rider.ro" "rider|rider_normalConstraint1.cro";
connectAttr "rider|rider_geometryConstraint1.w0" "rider|rider_geometryConstraint1.tg[0].tw";
connectAttr "rider|rider_normalConstraint1.crx" "rider.rx";
connectAttr "rider|rider_normalConstraint1.cry" "rider.ry";
connectAttr "rider|rider_normalConstraint1.crz" "rider.rz";
connectAttr "rider|rider_normalConstraint1.w0" "rider|rider_normalConstraint1.tg[0].tw";
connectAttr "scaleFollower.pim" "scaleFollower|scaleFollower_scaleConstraint1.cpim";
connectAttr "scaleFollower|scaleFollower_scaleConstraint1.csx" "scaleFollower.sx";
connectAttr "scaleFollower|scaleFollower_scaleConstraint1.csy" "scaleFollower.sy";
connectAttr "scaleFollower|scaleFollower_scaleConstraint1.csz" "scaleFollower.sz";
connectAttr "scaleFollower|scaleFollower_scaleConstraint1.w0" "scaleFollower|scaleFollower_scaleConstraint1.tg[0].tw";
connectAttr "aimFollowerShape.iog" "followerMatSG.dsm" -na;
connectAttr "fillLight.iog" ":defaultLightSet.dsm" -na;
connectAttr "fillLightShape.ltd" ":lightList1.l" -na;
connectAttr "followerMat.msg" ":defaultShaderList1.s" -na;
connectAttr "followerMatSG.pa" ":renderPartition.st" -na;
connectAttr "hillMat.msg" ":defaultShaderList1.s" -na;
connectAttr "hillMatSG.pa" ":renderPartition.st" -na;
connectAttr "hillShape.iog" "hillMatSG.dsm" -na;
connectAttr "keyLight.iog" ":defaultLightSet.dsm" -na;
connectAttr "keyLightShape.ltd" ":lightList1.l" -na;
connectAttr "orientFollowerShape.iog" "followerMatSG.dsm" -na;
connectAttr "parentFollowerShape.iog" "followerMatSG.dsm" -na;
connectAttr "pointFollowerShape.iog" "followerMatSG.dsm" -na;
connectAttr "riderShape.iog" "followerMatSG.dsm" -na;
connectAttr "scaleFollowerShape.iog" "followerMatSG.dsm" -na;
// End of constraints.ma
