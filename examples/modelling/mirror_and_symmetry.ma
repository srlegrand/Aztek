//Maya ASCII 2024 scene
//Name: mirror_and_symmetry.ma
//Codeset: UTF-8
requires maya "2024";
currentUnit -l centimeter -a degree -t film;
fileInfo "application" "aztek";
fileInfo "version" "1.0.0";
createNode transform -s -n "persp";
	setAttr ".v" no;
	setAttr ".t" -type "double3" 6 6 8 ;
	setAttr ".r" -type "double3" -25.641005824305278 36.86989764584401 0 ;
createNode camera -s -n "perspShape" -p "persp";
	setAttr ".coi" 11.092339699089637;
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
createNode transform -n "ship";
	setAttr ".t" -type "double3" 0.75 1.2 0 ;
createNode mesh -n "shipShape" -p "ship";
	setAttr -k off ".v";
	setAttr ".vir" yes;
	setAttr ".vif" yes;
	setAttr ".uvst[0].uvsn" -type "string" "map1";
	setAttr -s 132 ".uvst[0].uvsp";
	setAttr ".uvst[0].uvsp[0:131]" -type "float2" 0.375 0 0.625 0 0.625 0.25 0.375 0.25 0.375 0.25 0.625 0.25 0.625 0.33333334 0.375 0.33333334 0.375 0.33333334 0.625 0.33333334 0.625 0.4166667 0.375 0.4166667 0.625 0.5 0.375 0.5 0.625 0.75 0.375 0.75 0.625 0.8333333 0.375 0.8333333 0.375 0.8333333 0.625 0.8333333 0.625 0.9166667 0.375 0.9166667 0.375 0.9166667 0.625 0.9166667 0.625 1 0.375 1 0.7083333 0 0.7083333 0.25 0.7916667 0 0.7916667 0.25 0.875 0 0.875 0.25 0.125 0 0.20833334 0 0.20833334 0.25 0.125 0.25 0.2916667 0 0.2916667 0.25 0.375 0.25 0.625 0.25 0.625 0 0.375 0 0.375 0.33333334 0.625 0.33333334 0.375 0.4166667 0.625 0.4166667 0.375 0.5 0.625 0.5 0.375 0.75 0.625 0.75 0.375 0.8333333 0.625 0.8333333 0.375 0.9166667 0.625 0.9166667 0.375 1 0.625 1 0.7083333 0.25 0.7083333 0 0.7916667 0.25 0.7916667 0 0.875 0.25 0.875 0 0.125 0.25 0.20833334 0.25 0.20833334 0 0.125 0 0.2916667 0.25 0.2916667 0 0.375 0.25 0.375 0 0.375 0 0.375 0.25 0.625 0.25 0.625 0 0.375 0.25 0.375 0.33333334 0.625 0.33333334 0.625 0.25 0.375 0.33333334 0.375 0.4166667 0.625 0.4166667 0.625 0.33333334 0.375 0.5 0.625 0.5 0.375 0.75 0.625 0.75 0.375 0.8333333 0.625 0.8333333 0.375 0.8333333 0.375 0.9166667 0.625 0.9166667 0.625 0.8333333 0.375 0.9166667 0.375 1 0.625 1 0.625 0.9166667 0.7083333 0.25 0.7083333 0 0.7916667 0.25 0.7916667 0 0.875 0.25 0.875 0 0.125 0 0.125 0.25 0.20833334 0.25 0.20833334 0 0.2916667 0.25 0.2916667 0 0.625 0.25 0.625 0 0.7083333 0 0.7083333 0.25 0.7916667 0 0.7916667 0.25 0.875 0 0.875 0.25 0.375 0.25 0.375 0 0.375 0.33333334 0.625 0.33333334 0.375 0.4166667 0.625 0.4166667 0.375 0.5 0.625 0.5 0.375 0.75 0.625 0.75 0.375 0.8333333 0.625 0.8333333 0.375 0.9166667 0.625 0.9166667 0.375 1 0.625 1 ;
	setAttr ".cuvs" -type "string" "map1";
	setAttr ".dcc" -type "string" "Ambient+Diffuse";
	setAttr ".covm[0]"  0 1 1;
	setAttr ".cdvm[0]"  0 1 1;
	setAttr -s 72 ".vt";
	setAttr ".vt[0:71]"  -0.5627776749732568 -1.8514660157990384 3.86872487564239 0.5627776749732568 -1.8514660157990384 3.86872487564239 0.5627776749732568 1.903878681797873 3.993729414813438 -0.5627776749732568 1.903878681797873 3.993729414813438 0.6471194077712559 2.1329401894581426 1.1147665110050156 -0.6471194077712559 2.1329401894581426 1.1147665110050156 0.6471194077712559 2.0781456689101976 -0.40578143420046386 -0.6471194077712559 2.0781456689101976 -0.40578143420046386 0.5627776749732568 1.6407919508460032 -3.306927369100964 -0.5627776749732568 1.6407919508460032 -3.306927369100964 0.5627776749732568 -2.114552746750908 -3.431931908272012 -0.5627776749732568 -2.114552746750908 -3.431931908272012 0.6471194077712559 -2.3436142544111775 -0.5529690044635888 -0.6471194077712559 -2.3436142544111775 -0.5529690044635888 0.6471194077712559 -2.2888197338632326 0.967578940741891 -0.6471194077712559 -2.2888197338632326 0.967578940741891 -0.75 -0.4 2.5 0.75 -0.4 2.5 0.75 0.4 2.5 -0.75 0.40000000000000013 2.5 0.75 0.4 0.8333333333333335 -0.75 0.40000000000000013 0.8333333333333335 0.75 0.4 -0.8333333333333333 -0.75 0.40000000000000013 -0.8333333333333333 0.75 0.4 -2.5 -0.75 0.40000000000000013 -2.5 0.75 -0.4 -2.5 -0.75 -0.4 -2.5 0.75 -0.4 -0.8333333333333333 -0.75 -0.4 -0.8333333333333333 0.75 -0.4 0.8333333333333335 -0.75 -0.4 0.8333333333333335 -0.3939443724812797 2.436662779005077 3.601541604295915 0.3939443724812797 2.436662779005077 3.601541604295915 0.4529835854398791 2.5970058343672657 1.5862675716300192 -0.4529835854398791 2.5970058343672657 1.5862675716300192 -0.3882716446627535 -2.729656157774822 -0.15842695389192585 0.3882716446627535 -2.729656157774822 -0.15842695389192585 0.3882716446627535 -2.7022588975008492 0.601847018710814 -0.3882716446627535 -2.7022588975008492 0.601847018710814 -0.9372223250267432 -1.8514660157990384 3.86872487564239 -2.062777674973257 -1.8514660157990384 3.86872487564239 -2.062777674973257 1.903878681797873 3.993729414813438 -0.9372223250267432 1.903878681797873 3.993729414813438 -2.147119407771256 2.1329401894581426 1.1147665110050156 -0.8528805922287441 2.1329401894581426 1.1147665110050156 -2.147119407771256 2.0781456689101976 -0.40578143420046386 -0.8528805922287441 2.0781456689101976 -0.40578143420046386 -2.062777674973257 1.6407919508460032 -3.306927369100964 -0.9372223250267432 1.6407919508460032 -3.306927369100964 -2.062777674973257 -2.114552746750908 -3.431931908272012 -0.9372223250267432 -2.114552746750908 -3.431931908272012 -2.147119407771256 -2.3436142544111775 -0.5529690044635888 -0.8528805922287441 -2.3436142544111775 -0.5529690044635888 -2.147119407771256 -2.2888197338632326 0.967578940741891 -0.8528805922287441 -2.2888197338632326 0.967578940741891 -2.25 -0.4 2.5 -2.25 0.4 2.5 -2.25 0.4 0.8333333333333335 -2.25 0.4 -0.8333333333333333 -2.25 0.4 -2.5 -2.25 -0.4 -2.5 -2.25 -0.4 -0.8333333333333333 -2.25 -0.4 0.8333333333333335 -1.1060556275187203 2.436662779005077 3.601541604295915 -1.8939443724812797 2.436662779005077 3.601541604295915 -1.9529835854398792 2.5970058343672657 1.5862675716300192 -1.0470164145601208 2.5970058343672657 1.5862675716300192 -1.1117283553372466 -2.729656157774822 -0.15842695389192585 -1.8882716446627534 -2.729656157774822 -0.15842695389192585 -1.8882716446627534 -2.7022588975008492 0.601847018710814 -1.1117283553372466 -2.7022588975008492 0.601847018710814;
	setAttr -s 134 ".ed";
	setAttr ".ed[0:133]"  0 1 0 1 2 0 2 3 0 3 0 0 32 33 0 33 34 0 34 35 1 35 32 0 5 4 1 4 6 0 6 7 1 7 5 0 6 8 0 8 9 0 9 7 0 8 10 0 10 11 0 11 9 0 10 12 0 12 13 1 13 11 0 36 37 1 37 38 0 38 39 1 39 36 0 15 14 1 14 1 0 0 15 0 14 4 1 4 2 0 14 12 0 12 6 1 13 7 1 13 15 0 15 5 1 3 5 0 19 18 1 18 17 1 17 16 1 16 19 1 21 20 1 20 18 1 19 21 1 23 22 1 22 20 1 21 23 1 25 24 1 24 22 1 23 25 1 27 26 1 26 24 1 25 27 1 29 28 1 28 26 1 27 29 1 31 30 1 30 28 1 29 31 1 17 30 1 31 16 1 20 30 1 22 28 1 23 29 1 21 31 1 2 33 0 32 3 0 4 34 0 5 35 0 12 37 0 36 13 0 14 38 0 15 39 0 40 43 0 43 42 0 42 41 0 41 40 0 64 67 0 67 66 1 66 65 0 65 64 0 45 47 0 47 46 1 46 44 0 44 45 1 47 49 0 49 48 0 48 46 0 49 51 0 51 50 0 50 48 0 51 53 0 53 52 1 52 50 0 68 71 0 71 70 1 70 69 0 69 68 1 55 40 0 41 54 0 54 55 1 42 44 0 44 54 1 46 52 1 52 54 0 47 53 1 45 55 1 55 53 0 45 43 0 57 56 1 56 63 1 63 58 1 58 57 1 63 62 1 62 59 1 59 58 1 62 61 1 61 60 1 60 59 1 43 64 0 65 42 0 66 44 0 67 45 0 53 68 0 69 52 0 70 54 0 71 55 0 16 56 1 57 19 1 58 21 1 59 23 1 60 25 1 61 27 1 62 29 1 63 31 1;
	setAttr -s 69 -ch 276 ".fc[0:68]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 
		f 4 4 5 6 7 mu 0 4 4 5 6 7 
		f 4 8 9 10 11 mu 0 4 8 9 10 11 
		f 4 -11 12 13 14 mu 0 4 11 10 12 13 
		f 4 -14 15 16 17 mu 0 4 13 12 14 15 
		f 4 -17 18 19 20 mu 0 4 15 14 16 17 
		f 4 21 22 23 24 mu 0 4 18 19 20 21 
		f 4 25 26 -1 27 mu 0 4 22 23 24 25 
		f 4 -27 28 29 -2 mu 0 4 1 26 27 2 
		f 4 30 31 -10 -29 mu 0 4 26 28 29 27 
		f 4 -19 -16 -13 -32 mu 0 4 28 30 31 29 
		f 4 -21 32 -15 -18 mu 0 4 32 33 34 35 
		f 4 33 34 -12 -33 mu 0 4 33 36 37 34 
		f 4 -28 -4 35 -35 mu 0 4 36 0 3 37 
		f 4 36 37 38 39 mu 0 4 38 39 40 41 
		f 4 40 41 -37 42 mu 0 4 42 43 39 38 
		f 4 43 44 -41 45 mu 0 4 44 45 43 42 
		f 4 46 47 -44 48 mu 0 4 46 47 45 44 
		f 4 49 50 -47 51 mu 0 4 48 49 47 46 
		f 4 52 53 -50 54 mu 0 4 50 51 49 48 
		f 4 55 56 -53 57 mu 0 4 52 53 51 50 
		f 4 -39 58 -56 59 mu 0 4 54 55 53 52 
		f 4 -42 60 -59 -38 mu 0 4 39 56 57 40 
		f 4 -45 61 -57 -61 mu 0 4 56 58 59 57 
		f 4 -48 -51 -54 -62 mu 0 4 58 60 61 59 
		f 4 -49 62 -55 -52 mu 0 4 62 63 64 65 
		f 4 -46 63 -58 -63 mu 0 4 63 66 67 64 
		f 4 -43 -40 -60 -64 mu 0 4 66 68 69 67 
		f 4 -3 64 -5 65 mu 0 4 3 2 5 4 
		f 4 -30 66 -6 -65 mu 0 4 2 9 6 5 
		f 4 -9 67 -7 -67 mu 0 4 9 8 7 6 
		f 4 -36 -66 -8 -68 mu 0 4 8 3 4 7 
		f 4 -20 68 -22 69 mu 0 4 17 16 19 18 
		f 4 -31 70 -23 -69 mu 0 4 16 23 20 19 
		f 4 -26 71 -24 -71 mu 0 4 23 22 21 20 
		f 4 -34 -70 -25 -72 mu 0 4 22 17 18 21 
		f 4 72 73 74 75 mu 0 4 70 71 72 73 
		f 4 76 77 78 79 mu 0 4 74 75 76 77 
		f 4 80 81 82 83 mu 0 4 78 79 80 81 
		f 4 84 85 86 -82 mu 0 4 79 82 83 80 
		f 4 87 88 89 -86 mu 0 4 82 84 85 83 
		f 4 90 91 92 -89 mu 0 4 84 86 87 85 
		f 4 93 94 95 96 mu 0 4 88 89 90 91 
		f 4 97 -76 98 99 mu 0 4 92 93 94 95 
		f 4 -75 100 101 -99 mu 0 4 73 72 96 97 
		f 4 -102 -83 102 103 mu 0 4 97 96 98 99 
		f 4 -103 -87 -90 -93 mu 0 4 99 98 100 101 
		f 4 -88 -85 104 -91 mu 0 4 102 103 104 105 
		f 4 -105 -81 105 106 mu 0 4 105 104 106 107 
		f 4 -106 107 -73 -98 mu 0 4 107 106 71 70 
		f 4 108 109 110 111 mu 0 4 108 109 110 111 
		f 4 -111 112 113 114 mu 0 4 111 110 112 113 
		f 4 -114 115 116 117 mu 0 4 113 112 114 115 
		f 4 118 -80 119 -74 mu 0 4 71 74 77 72 
		f 4 -120 -79 120 -101 mu 0 4 72 77 76 81 
		f 4 -121 -78 121 -84 mu 0 4 81 76 75 78 
		f 4 -122 -77 -119 -108 mu 0 4 78 75 74 71 
		f 4 122 -97 123 -92 mu 0 4 86 88 91 87 
		f 4 -124 -96 124 -104 mu 0 4 87 91 90 95 
		f 4 -125 -95 125 -100 mu 0 4 95 90 89 92 
		f 4 -126 -94 -123 -107 mu 0 4 92 89 88 86 
		f 4 -40 126 -109 127 mu 0 4 116 117 109 108 
		f 4 -43 -128 -112 128 mu 0 4 118 116 108 119 
		f 4 -46 -129 -115 129 mu 0 4 120 118 119 121 
		f 4 -49 -130 -118 130 mu 0 4 122 120 121 123 
		f 4 -52 -131 -117 131 mu 0 4 124 122 123 125 
		f 4 -55 -132 -116 132 mu 0 4 126 124 125 127 
		f 4 -58 -133 -113 133 mu 0 4 128 126 127 129 
		f 4 -60 -134 -110 -127 mu 0 4 130 128 129 131 ;
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
createNode blinn -n "shipMat";
	setAttr ".c" -type "float3" 0.8 0.82 0.88 ;
createNode shadingEngine -n "shipMatSG";
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
relationship "link" ":lightLinker1" "shipMatSG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "floorMatSG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" ":initialShadingGroup.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "shipMatSG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "floorMatSG.message" ":defaultLightSet.message";
connectAttr "floorMat.oc" "floorMatSG.ss";
connectAttr "shipMat.oc" "shipMatSG.ss";
connectAttr "fillLight.iog" ":defaultLightSet.dsm" -na;
connectAttr "fillLightShape.ltd" ":lightList1.l" -na;
connectAttr "floorMat.msg" ":defaultShaderList1.s" -na;
connectAttr "floorMatSG.pa" ":renderPartition.st" -na;
connectAttr "floorShape.iog" "floorMatSG.dsm" -na;
connectAttr "keyLight.iog" ":defaultLightSet.dsm" -na;
connectAttr "keyLightShape.ltd" ":lightList1.l" -na;
connectAttr "shipMat.msg" ":defaultShaderList1.s" -na;
connectAttr "shipMatSG.pa" ":renderPartition.st" -na;
connectAttr "shipShape.iog" "shipMatSG.dsm" -na;
// End of mirror_and_symmetry.ma
