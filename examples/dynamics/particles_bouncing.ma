//Maya ASCII 2024 scene
//Name: particles_bouncing.ma
//Codeset: UTF-8
requires maya "2024";
requires "aztekDynamics" "1.0";
currentUnit -l centimeter -a degree -t film;
fileInfo "application" "aztek";
fileInfo "version" "1.0.0";
createNode transform -s -n "persp";
	setAttr ".v" no;
	setAttr ".t" -type "double3" 150 450 900 ;
	setAttr ".r" -type "double3" -12.363616828670864 9.46232220802562 0 ;
createNode camera -s -n "perspShape" -p "persp";
	setAttr ".coi" 934.0770846134702;
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
createNode transform -n "ramp";
	setAttr ".t" -type "double3" 0 300 0 ;
	setAttr ".r" -type "double3" 0 0 -20 ;
createNode mesh -n "rampShape" -p "ramp";
	setAttr -k off ".v";
	setAttr ".vir" yes;
	setAttr ".vif" yes;
	setAttr ".uvst[0].uvsn" -type "string" "map1";
	setAttr -s 121 ".uvst[0].uvsp";
	setAttr ".uvst[0].uvsp[0:120]" -type "float2" 0 0 0.1 0 0.1 0.1 0 0.1 0.2 0 0.2 0.1 0.3 0 0.3 0.1 0.4 0 0.4 0.1 0.5 0 0.5 0.1 0.6 0 0.6 0.1 0.7 0 0.7 0.1 0.8 0 0.8 0.1 0.9 0 0.9 0.1 1 0 1 0.1 0.1 0.2 0 0.2 0.2 0.2 0.3 0.2 0.4 0.2 0.5 0.2 0.6 0.2 0.7 0.2 0.8 0.2 0.9 0.2 1 0.2 0.1 0.3 0 0.3 0.2 0.3 0.3 0.3 0.4 0.3 0.5 0.3 0.6 0.3 0.7 0.3 0.8 0.3 0.9 0.3 1 0.3 0.1 0.4 0 0.4 0.2 0.4 0.3 0.4 0.4 0.4 0.5 0.4 0.6 0.4 0.7 0.4 0.8 0.4 0.9 0.4 1 0.4 0.1 0.5 0 0.5 0.2 0.5 0.3 0.5 0.4 0.5 0.5 0.5 0.6 0.5 0.7 0.5 0.8 0.5 0.9 0.5 1 0.5 0.1 0.6 0 0.6 0.2 0.6 0.3 0.6 0.4 0.6 0.5 0.6 0.6 0.6 0.7 0.6 0.8 0.6 0.9 0.6 1 0.6 0.1 0.7 0 0.7 0.2 0.7 0.3 0.7 0.4 0.7 0.5 0.7 0.6 0.7 0.7 0.7 0.8 0.7 0.9 0.7 1 0.7 0.1 0.8 0 0.8 0.2 0.8 0.3 0.8 0.4 0.8 0.5 0.8 0.6 0.8 0.7 0.8 0.8 0.8 0.9 0.8 1 0.8 0.1 0.9 0 0.9 0.2 0.9 0.3 0.9 0.4 0.9 0.5 0.9 0.6 0.9 0.7 0.9 0.8 0.9 0.9 0.9 1 0.9 0.1 1 0 1 0.2 1 0.3 1 0.4 1 0.5 1 0.6 1 0.7 1 0.8 1 0.9 1 1 1 ;
	setAttr ".cuvs" -type "string" "map1";
	setAttr ".dcc" -type "string" "Ambient+Diffuse";
	setAttr ".covm[0]"  0 1 1;
	setAttr ".cdvm[0]"  0 1 1;
	setAttr -s 121 ".vt";
	setAttr ".vt[0:120]"  -400 0 200 -320 0 200 -240 0 200 -160 0 200 -79.99999999999999 0 200 0 0 200 79.99999999999999 0 200 159.99999999999997 0 200 240.00000000000003 0 200 320 0 200 400 0 200 -400 0 160 -320 0 160 -240 0 160 -160 0 160 -79.99999999999999 0 160 0 0 160 79.99999999999999 0 160 159.99999999999997 0 160 240.00000000000003 0 160 320 0 160 400 0 160 -400 0 120 -320 0 120 -240 0 120 -160 0 120 -79.99999999999999 0 120 0 0 120 79.99999999999999 0 120 159.99999999999997 0 120 240.00000000000003 0 120 320 0 120 400 0 120 -400 0 80 -320 0 80 -240 0 80 -160 0 80 -79.99999999999999 0 80 0 0 80 79.99999999999999 0 80 159.99999999999997 0 80 240.00000000000003 0 80 320 0 80 400 0 80 -400 0 39.99999999999999 -320 0 39.99999999999999 -240 0 39.99999999999999 -160 0 39.99999999999999 -79.99999999999999 0 39.99999999999999 0 0 39.99999999999999 79.99999999999999 0 39.99999999999999 159.99999999999997 0 39.99999999999999 240.00000000000003 0 39.99999999999999 320 0 39.99999999999999 400 0 39.99999999999999 -400 0 0 -320 0 0 -240 0 0 -160 0 0 -79.99999999999999 0 0 0 0 0 79.99999999999999 0 0 159.99999999999997 0 0 240.00000000000003 0 0 320 0 0 400 0 0 -400 0 -39.99999999999999 -320 0 -39.99999999999999 -240 0 -39.99999999999999 -160 0 -39.99999999999999 -79.99999999999999 0 -39.99999999999999 0 0 -39.99999999999999 79.99999999999999 0 -39.99999999999999 159.99999999999997 0 -39.99999999999999 240.00000000000003 0 -39.99999999999999 320 0 -39.99999999999999 400 0 -39.99999999999999 -400 0 -79.99999999999999 -320 0 -79.99999999999999 -240 0 -79.99999999999999 -160 0 -79.99999999999999 -79.99999999999999 0 -79.99999999999999 0 0 -79.99999999999999 79.99999999999999 0 -79.99999999999999 159.99999999999997 0 -79.99999999999999 240.00000000000003 0 -79.99999999999999 320 0 -79.99999999999999 400 0 -79.99999999999999 -400 0 -120.00000000000001 -320 0 -120.00000000000001 -240 0 -120.00000000000001 -160 0 -120.00000000000001 -79.99999999999999 0 -120.00000000000001 0 0 -120.00000000000001 79.99999999999999 0 -120.00000000000001 159.99999999999997 0 -120.00000000000001 240.00000000000003 0 -120.00000000000001 320 0 -120.00000000000001 400 0 -120.00000000000001 -400 0 -160 -320 0 -160 -240 0 -160 -160 0 -160 -79.99999999999999 0 -160 0 0 -160 79.99999999999999 0 -160 159.99999999999997 0 -160 240.00000000000003 0 -160 320 0 -160 400 0 -160 -400 0 -200 -320 0 -200 -240 0 -200 -160 0 -200 -79.99999999999999 0 -200 0 0 -200 79.99999999999999 0 -200 159.99999999999997 0 -200 240.00000000000003 0 -200 320 0 -200 400 0 -200;
	setAttr -s 220 ".ed";
	setAttr ".ed[0:165]"  0 1 1 1 12 1 12 11 1 11 0 1 1 2 1 2 13 1 13 12 1 2 3 1 3 14 1 14 13 1 3 4 1 4 15 1 15 14 1 4 5 1 5 16 1 16 15 1 5 6 1 6 17 1 17 16 1 6 7 1 7 18 1 18 17 1 7 8 1 8 19 1 19 18 1 8 9 1 9 20 1 20 19 1 9 10 1 10 21 1 21 20 1 12 23 1 23 22 1 22 11 1 13 24 1 24 23 1 14 25 1 25 24 1 15 26 1 26 25 1 16 27 1 27 26 1 17 28 1 28 27 1 18 29 1 29 28 1 19 30 1 30 29 1 20 31 1 31 30 1 21 32 1 32 31 1 23 34 1 34 33 1 33 22 1 24 35 1 35 34 1 25 36 1 36 35 1 26 37 1 37 36 1 27 38 1 38 37 1 28 39 1 39 38 1 29 40 1 40 39 1 30 41 1 41 40 1 31 42 1 42 41 1 32 43 1 43 42 1 34 45 1 45 44 1 44 33 1 35 46 1 46 45 1 36 47 1 47 46 1 37 48 1 48 47 1 38 49 1 49 48 1 39 50 1 50 49 1 40 51 1 51 50 1 41 52 1 52 51 1 42 53 1 53 52 1 43 54 1 54 53 1 45 56 1 56 55 1 55 44 1 46 57 1 57 56 1 47 58 1 58 57 1 48 59 1 59 58 1 49 60 1 60 59 1 50 61 1 61 60 1 51 62 1 62 61 1 52 63 1 63 62 1 53 64 1 64 63 1 54 65 1 65 64 1 56 67 1 67 66 1 66 55 1 57 68 1 68 67 1 58 69 1 69 68 1 59 70 1 70 69 1 60 71 1 71 70 1 61 72 1 72 71 1 62 73 1 73 72 1 63 74 1 74 73 1 64 75 1 75 74 1 65 76 1 76 75 1 67 78 1 78 77 1 77 66 1 68 79 1 79 78 1 69 80 1 80 79 1 70 81 1 81 80 1 71 82 1 82 81 1 72 83 1 83 82 1 73 84 1 84 83 1 74 85 1 85 84 1 75 86 1 86 85 1 76 87 1 87 86 1 78 89 1 89 88 1 88 77 1 79 90 1 90 89 1 80 91 1 91 90 1 81 92 1 92 91 1;
	setAttr ".ed[166:219]"  82 93 1 93 92 1 83 94 1 94 93 1 84 95 1 95 94 1 85 96 1 96 95 1 86 97 1 97 96 1 87 98 1 98 97 1 89 100 1 100 99 1 99 88 1 90 101 1 101 100 1 91 102 1 102 101 1 92 103 1 103 102 1 93 104 1 104 103 1 94 105 1 105 104 1 95 106 1 106 105 1 96 107 1 107 106 1 97 108 1 108 107 1 98 109 1 109 108 1 100 111 1 111 110 1 110 99 1 101 112 1 112 111 1 102 113 1 113 112 1 103 114 1 114 113 1 104 115 1 115 114 1 105 116 1 116 115 1 106 117 1 117 116 1 107 118 1 118 117 1 108 119 1 119 118 1 109 120 1 120 119 1;
	setAttr -s 100 -ch 400 ".fc[0:99]" -type "polyFaces" 
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
		f 4 -3 31 32 33 mu 0 4 3 2 22 23 
		f 4 -7 34 35 -32 mu 0 4 2 5 24 22 
		f 4 -10 36 37 -35 mu 0 4 5 7 25 24 
		f 4 -13 38 39 -37 mu 0 4 7 9 26 25 
		f 4 -16 40 41 -39 mu 0 4 9 11 27 26 
		f 4 -19 42 43 -41 mu 0 4 11 13 28 27 
		f 4 -22 44 45 -43 mu 0 4 13 15 29 28 
		f 4 -25 46 47 -45 mu 0 4 15 17 30 29 
		f 4 -28 48 49 -47 mu 0 4 17 19 31 30 
		f 4 -31 50 51 -49 mu 0 4 19 21 32 31 
		f 4 -33 52 53 54 mu 0 4 23 22 33 34 
		f 4 -36 55 56 -53 mu 0 4 22 24 35 33 
		f 4 -38 57 58 -56 mu 0 4 24 25 36 35 
		f 4 -40 59 60 -58 mu 0 4 25 26 37 36 
		f 4 -42 61 62 -60 mu 0 4 26 27 38 37 
		f 4 -44 63 64 -62 mu 0 4 27 28 39 38 
		f 4 -46 65 66 -64 mu 0 4 28 29 40 39 
		f 4 -48 67 68 -66 mu 0 4 29 30 41 40 
		f 4 -50 69 70 -68 mu 0 4 30 31 42 41 
		f 4 -52 71 72 -70 mu 0 4 31 32 43 42 
		f 4 -54 73 74 75 mu 0 4 34 33 44 45 
		f 4 -57 76 77 -74 mu 0 4 33 35 46 44 
		f 4 -59 78 79 -77 mu 0 4 35 36 47 46 
		f 4 -61 80 81 -79 mu 0 4 36 37 48 47 
		f 4 -63 82 83 -81 mu 0 4 37 38 49 48 
		f 4 -65 84 85 -83 mu 0 4 38 39 50 49 
		f 4 -67 86 87 -85 mu 0 4 39 40 51 50 
		f 4 -69 88 89 -87 mu 0 4 40 41 52 51 
		f 4 -71 90 91 -89 mu 0 4 41 42 53 52 
		f 4 -73 92 93 -91 mu 0 4 42 43 54 53 
		f 4 -75 94 95 96 mu 0 4 45 44 55 56 
		f 4 -78 97 98 -95 mu 0 4 44 46 57 55 
		f 4 -80 99 100 -98 mu 0 4 46 47 58 57 
		f 4 -82 101 102 -100 mu 0 4 47 48 59 58 
		f 4 -84 103 104 -102 mu 0 4 48 49 60 59 
		f 4 -86 105 106 -104 mu 0 4 49 50 61 60 
		f 4 -88 107 108 -106 mu 0 4 50 51 62 61 
		f 4 -90 109 110 -108 mu 0 4 51 52 63 62 
		f 4 -92 111 112 -110 mu 0 4 52 53 64 63 
		f 4 -94 113 114 -112 mu 0 4 53 54 65 64 
		f 4 -96 115 116 117 mu 0 4 56 55 66 67 
		f 4 -99 118 119 -116 mu 0 4 55 57 68 66 
		f 4 -101 120 121 -119 mu 0 4 57 58 69 68 
		f 4 -103 122 123 -121 mu 0 4 58 59 70 69 
		f 4 -105 124 125 -123 mu 0 4 59 60 71 70 
		f 4 -107 126 127 -125 mu 0 4 60 61 72 71 
		f 4 -109 128 129 -127 mu 0 4 61 62 73 72 
		f 4 -111 130 131 -129 mu 0 4 62 63 74 73 
		f 4 -113 132 133 -131 mu 0 4 63 64 75 74 
		f 4 -115 134 135 -133 mu 0 4 64 65 76 75 
		f 4 -117 136 137 138 mu 0 4 67 66 77 78 
		f 4 -120 139 140 -137 mu 0 4 66 68 79 77 
		f 4 -122 141 142 -140 mu 0 4 68 69 80 79 
		f 4 -124 143 144 -142 mu 0 4 69 70 81 80 
		f 4 -126 145 146 -144 mu 0 4 70 71 82 81 
		f 4 -128 147 148 -146 mu 0 4 71 72 83 82 
		f 4 -130 149 150 -148 mu 0 4 72 73 84 83 
		f 4 -132 151 152 -150 mu 0 4 73 74 85 84 
		f 4 -134 153 154 -152 mu 0 4 74 75 86 85 
		f 4 -136 155 156 -154 mu 0 4 75 76 87 86 
		f 4 -138 157 158 159 mu 0 4 78 77 88 89 
		f 4 -141 160 161 -158 mu 0 4 77 79 90 88 
		f 4 -143 162 163 -161 mu 0 4 79 80 91 90 
		f 4 -145 164 165 -163 mu 0 4 80 81 92 91 
		f 4 -147 166 167 -165 mu 0 4 81 82 93 92 
		f 4 -149 168 169 -167 mu 0 4 82 83 94 93 
		f 4 -151 170 171 -169 mu 0 4 83 84 95 94 
		f 4 -153 172 173 -171 mu 0 4 84 85 96 95 
		f 4 -155 174 175 -173 mu 0 4 85 86 97 96 
		f 4 -157 176 177 -175 mu 0 4 86 87 98 97 
		f 4 -159 178 179 180 mu 0 4 89 88 99 100 
		f 4 -162 181 182 -179 mu 0 4 88 90 101 99 
		f 4 -164 183 184 -182 mu 0 4 90 91 102 101 
		f 4 -166 185 186 -184 mu 0 4 91 92 103 102 
		f 4 -168 187 188 -186 mu 0 4 92 93 104 103 
		f 4 -170 189 190 -188 mu 0 4 93 94 105 104 
		f 4 -172 191 192 -190 mu 0 4 94 95 106 105 
		f 4 -174 193 194 -192 mu 0 4 95 96 107 106 
		f 4 -176 195 196 -194 mu 0 4 96 97 108 107 
		f 4 -178 197 198 -196 mu 0 4 97 98 109 108 
		f 4 -180 199 200 201 mu 0 4 100 99 110 111 
		f 4 -183 202 203 -200 mu 0 4 99 101 112 110 
		f 4 -185 204 205 -203 mu 0 4 101 102 113 112 
		f 4 -187 206 207 -205 mu 0 4 102 103 114 113 
		f 4 -189 208 209 -207 mu 0 4 103 104 115 114 
		f 4 -191 210 211 -209 mu 0 4 104 105 116 115 
		f 4 -193 212 213 -211 mu 0 4 105 106 117 116 
		f 4 -195 214 215 -213 mu 0 4 106 107 118 117 
		f 4 -197 216 217 -215 mu 0 4 107 108 119 118 
		f 4 -199 218 219 -217 mu 0 4 108 109 120 119 ;
createNode transform -n "solver1";
createNode solver -n "solverShape1" -p "solver1";
	setAttr -k off ".v";
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
	setAttr ".vt[0:3]"  -1500 0 1500 1500 0 1500 -1500 0 -1500 1500 0 -1500;
	setAttr -s 4 ".ed";
	setAttr ".ed[0:3]"  0 1 1 1 3 1 3 2 1 2 0 1;
	setAttr -s 1 -ch 4 ".fc[0]" -type "polyFaces" 
		f 4 0 1 2 3 mu 0 4 0 1 2 3 ;
createNode transform -n "fountain";
	setAttr ".t" -type "double3" -250 800 0 ;
createNode emitter -n "fountainShape" -p "fountain";
	setAttr -k off ".v";
	setAttr ".rate" 200;
	setAttr ".speed" 150;
createNode transform -n "particles1";
createNode particles -n "particlesShape1" -p "particles1";
	setAttr -k off ".v";
	setAttr ".lifespanMode" 1;
	setAttr ".lifespan" 5;
	setAttr ".radius" 9;
	setAttr ".bounce" 0.6;
	setAttr ".particleRenderType" 4;
createNode transform -n "keyLight";
	setAttr ".r" -type "double3" -45 35 0 ;
createNode directionalLight -n "keyLightShape" -p "keyLight";
	setAttr -k off ".v";
createNode transform -n "fillLight";
createNode ambientLight -n "fillLightShape" -p "fillLight";
	setAttr -k off ".v";
	setAttr ".in" 0.25;
createNode collider -n "collider1";
createNode lambert -n "floorMat";
	setAttr ".c" -type "float3" 0.45 0.45 0.45 ;
createNode shadingEngine -n "floorMatSG";
	setAttr ".ihi" 0;
	setAttr ".ro" yes;
createNode collider -n "collider2";
createNode lambert -n "rampMat";
	setAttr ".c" -type "float3" 0.3 0.5 0.8 ;
createNode shadingEngine -n "rampMatSG";
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
relationship "link" ":lightLinker1" "floorMatSG.message" ":defaultLightSet.message";
relationship "link" ":lightLinker1" "rampMatSG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" ":initialShadingGroup.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "floorMatSG.message" ":defaultLightSet.message";
relationship "shadowLink" ":lightLinker1" "rampMatSG.message" ":defaultLightSet.message";
connectAttr "floorMat.oc" "floorMatSG.ss";
connectAttr "floorShape.outMesh" "collider2.inputMesh";
connectAttr "floorShape.outMesh" "fountainShape.inputMesh";
connectAttr "floorShape.worldMatrix[0]" "collider2.inputMatrix";
connectAttr "floorShape.worldMatrix[0]" "fountainShape.inputMatrix";
connectAttr "fountainShape.message" "particlesShape1.newParticles[0]";
connectAttr "rampMat.oc" "rampMatSG.ss";
connectAttr "rampShape.outMesh" "collider1.inputMesh";
connectAttr "rampShape.worldMatrix[0]" "collider1.inputMatrix";
connectAttr "solverShape1.message" "collider1.solver";
connectAttr "solverShape1.message" "collider2.solver";
connectAttr "solverShape1.message" "particlesShape1.solver";
connectAttr "fillLight.iog" ":defaultLightSet.dsm" -na;
connectAttr "fillLightShape.ltd" ":lightList1.l" -na;
connectAttr "floorMat.msg" ":defaultShaderList1.s" -na;
connectAttr "floorMatSG.pa" ":renderPartition.st" -na;
connectAttr "floorShape.iog" "floorMatSG.dsm" -na;
connectAttr "keyLight.iog" ":defaultLightSet.dsm" -na;
connectAttr "keyLightShape.ltd" ":lightList1.l" -na;
connectAttr "rampMat.msg" ":defaultShaderList1.s" -na;
connectAttr "rampMatSG.pa" ":renderPartition.st" -na;
connectAttr "rampShape.iog" "rampMatSG.dsm" -na;
// End of particles_bouncing.ma
