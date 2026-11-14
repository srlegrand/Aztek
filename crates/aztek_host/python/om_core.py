# aztek.api.OpenAztek, the scene side: MFn, MObject, MObjectHandle, MDagPath, MPlug, MSelectionList, MGlobal, modifiers.
# An MObject holds a node's id, not its name, so it survives renames and reparenting.
import json as _json
import re as _re
import sys as _sys
import threading as _threading

import _aztek as _az

_script_cmds = _sys.modules["aztek.cmds"]


class _StoredCmds:
    """aztek.cmds in stored units, centimetres and radians, as the API's values are in the reference."""

    def __getattr__(self, name):
        # A compute or a solve answers what it can of its own node itself; the host can't be reached then, not even
        # to look the command up.
        ctx = getattr(_compute_local, "ctx", None)
        if ctx is not None and "command" in ctx:
            return lambda *args, **kwargs: ctx["command"](name, args, kwargs)
        fn = getattr(_script_cmds, name)

        def call(*args, **kwargs):
            was = _az.stored_units(True)
            try:
                return fn(*args, **kwargs)
            finally:
                _az.stored_units(was)

        return call

    # Plug-in commands come and go on the module itself.
    def __setattr__(self, name, value):
        setattr(_script_cmds, name, value)

    def __delattr__(self, name):
        delattr(_script_cmds, name)

    @property
    def __dict__(self):
        return _script_cmds.__dict__

    def __dir__(self):
        return dir(_script_cmds)


_cmds = _StoredCmds()


# Set per thread while a node computes: {"id", "name", "type", "attrs"}. The host can't be reached then, and a compute may
# run on a worker thread alongside others.
_compute_local = _threading.local()


def _api(op, **kw):
    ctx = getattr(_compute_local, "ctx", None)
    if ctx is not None:
        return _compute_answer(ctx, op, kw)
    return _json.loads(_az.api(op, _json.dumps(kw)))


def _compute_answer(ctx, op, kw):
    """What a compute can ask about its own node: its name, type and attributes."""
    if "query" in ctx:
        return ctx["query"](op, kw)
    if op == "node.exists":
        return kw["id"] == ctx["id"]
    if op == "node.info" and kw["id"] == ctx["id"]:
        return {
            "id": ctx["id"], "name": ctx["name"], "type": ctx["type"], "chain": [ctx["type"]], "dag": False, "owner": ctx["owner"],
            "path": ctx["name"], "full": ctx["name"], "parent": 0, "children": [], "intermediate": False,
        }
    if op == "node.find" and kw["name"] == ctx["name"]:
        return ctx["id"]
    if op == "plug.info" and kw["name"].split(".", 1)[0] == ctx["name"]:
        return _compute_plug_info(ctx, kw["name"].split(".", 1)[1])
    raise RuntimeError("(kFailure): Commands cannot run inside compute")


def _compute_plug_info(ctx, attr):
    element = -1
    if attr.endswith("]"):
        attr, _, i = attr[:-1].rpartition("[")
        element = int(i)
    for s in ctx["attrs"]:
        kids = s.get("children") or []
        if s["type"] == "vec3" and not kids:
            kids = [{"name": s["name"] + a, "short": s["short"] + a.lower()} for a in "XYZ"]
        names = [k["name"] for k in kids]
        shorts = [k["short"] for k in kids]
        component = -1
        if attr in names:
            component = names.index(attr)
        elif attr in shorts:
            component = shorts.index(attr)
        elif attr not in (s["name"], s["short"]):
            continue
        return {
            "long": s["name"], "short": s["short"], "type": s["type"], "array": bool(s.get("array")), "input": s.get("input", True),
            "output": not s.get("input", True), "keyable": bool(s.get("keyable")), "storable": s.get("storable", s.get("input", True)),
            "hidden": bool(s.get("hidden")), "channelBox": False, "user": False, "color": bool(s.get("color")), "children": names,
            "childShort": shorts, "enum": s.get("enum") or [], "min": s.get("min"), "max": s.get("max"), "softMin": s.get("softMin"),
            "softMax": s.get("softMax"), "nice": s.get("nice", ""), "default": s.get("default"), "node": ctx["id"], "component": component,
            "element": element, "name": ctx["name"] + "." + attr, "locked": False,
        }
    raise RuntimeError("(kInvalidParameter): The node has no attribute '%s'" % (attr,))


class MFn:
    """The reference's object kinds, with its numbers: its 2025 release as recorded by pymel's API cache, checked
    against its 2027 Python reference, which agrees on every value compared and adds kShotLabel. kLast is left out:
    the 2027 reference doesn't give it."""

    kInvalid = 0
    kBase = 1
    kNamedObject = 2
    kModel = 3
    kDependencyNode = 4
    kAddDoubleLinear = 5
    kAffect = 6
    kAnimCurve = 7
    kAnimCurveTimeToAngular = 8
    kAnimCurveTimeToDistance = 9
    kAnimCurveTimeToTime = 10
    kAnimCurveTimeToUnitless = 11
    kAnimCurveUnitlessToAngular = 12
    kAnimCurveUnitlessToDistance = 13
    kAnimCurveUnitlessToTime = 14
    kAnimCurveUnitlessToUnitless = 15
    kResultCurve = 16
    kResultCurveTimeToAngular = 17
    kResultCurveTimeToDistance = 18
    kResultCurveTimeToTime = 19
    kResultCurveTimeToUnitless = 20
    kAngleBetween = 21
    kAudio = 22
    kBackground = 23
    kColorBackground = 24
    kFileBackground = 25
    kRampBackground = 26
    kBlend = 27
    kBlendTwoAttr = 28
    kBlendWeighted = 29
    kBlendDevice = 30
    kBlendColors = 31
    kBump = 32
    kBump3d = 33
    kCameraView = 34
    kChainToSpline = 35
    kChoice = 36
    kCondition = 37
    kContrast = 38
    kClampColor = 39
    kCreate = 40
    kAlignCurve = 41
    kAlignSurface = 42
    kAttachCurve = 43
    kAttachSurface = 44
    kAvgCurves = 45
    kAvgSurfacePoints = 46
    kAvgNurbsSurfacePoints = 47
    kBevel = 48
    kBirailSrf = 49
    kDPbirailSrf = 50
    kMPbirailSrf = 51
    kSPbirailSrf = 52
    kBoundary = 53
    kCircle = 54
    kCloseCurve = 55
    kClosestPointOnSurface = 56
    kCloseSurface = 57
    kCurveFromSurface = 58
    kCurveFromSurfaceBnd = 59
    kCurveFromSurfaceCoS = 60
    kCurveFromSurfaceIso = 61
    kCurveInfo = 62
    kDetachCurve = 63
    kDetachSurface = 64
    kExtendCurve = 65
    kExtendSurface = 66
    kExtrude = 67
    kFFblendSrf = 68
    kFFfilletSrf = 69
    kFilletCurve = 70
    kFitBspline = 71
    kFlow = 72
    kHardenPointCurve = 73
    kIllustratorCurve = 74
    kInsertKnotCrv = 75
    kInsertKnotSrf = 76
    kIntersectSurface = 77
    kNurbsTesselate = 78
    kNurbsPlane = 79
    kNurbsCube = 80
    kOffsetCos = 81
    kOffsetCurve = 82
    kPlanarTrimSrf = 83
    kPointOnCurveInfo = 84
    kPointOnSurfaceInfo = 85
    kPrimitive = 86
    kProjectCurve = 87
    kProjectTangent = 88
    kRBFsurface = 89
    kRebuildCurve = 90
    kRebuildSurface = 91
    kReverseCurve = 92
    kReverseSurface = 93
    kRevolve = 94
    kRevolvedPrimitive = 95
    kCone = 96
    kRenderCone = 97
    kCylinder = 98
    kSphere = 99
    kSkin = 100
    kStitchSrf = 101
    kSubCurve = 102
    kSurfaceInfo = 103
    kTextCurves = 104
    kTrim = 105
    kUntrim = 106
    kDagNode = 107
    kProxy = 108
    kUnderWorld = 109
    kTransform = 110
    kAimConstraint = 111
    kLookAt = 112
    kGeometryConstraint = 113
    kGeometryVarGroup = 114
    kAnyGeometryVarGroup = 115
    kCurveVarGroup = 116
    kMeshVarGroup = 117
    kSurfaceVarGroup = 118
    kIkEffector = 119
    kIkHandle = 120
    kJoint = 121
    kManipulator3D = 122
    kArrowManip = 123
    kAxesActionManip = 124
    kBallProjectionManip = 125
    kCircleManip = 126
    kScreenAlignedCircleManip = 127
    kCircleSweepManip = 128
    kConcentricProjectionManip = 129
    kCubicProjectionManip = 130
    kCylindricalProjectionManip = 131
    kDiscManip = 132
    kFreePointManip = 133
    kCenterManip = 134
    kLimitManip = 135
    kEnableManip = 136
    kFreePointTriadManip = 137
    kPropMoveTriadManip = 138
    kTowPointManip = 139
    kPolyCreateToolManip = 140
    kPolySplitToolManip = 141
    kGeometryOnLineManip = 142
    kCameraPlaneManip = 143
    kToggleOnLineManip = 144
    kStateManip = 145
    kIsoparmManip = 146
    kLineManip = 147
    kManipContainer = 148
    kAverageCurveManip = 149
    kBarnDoorManip = 150
    kBevelManip = 151
    kBlendManip = 152
    kButtonManip = 153
    kCameraManip = 154
    kCoiManip = 155
    kCpManip = 156
    kCreateCVManip = 157
    kCreateEPManip = 158
    kCurveEdManip = 159
    kCurveSegmentManip = 160
    kDirectionManip = 161
    kDofManip = 162
    kDropoffManip = 163
    kExtendCurveDistanceManip = 164
    kExtrudeManip = 165
    kIkSplineManip = 166
    kIkRPManip = 167
    kJointClusterManip = 168
    kLightManip = 169
    kMotionPathManip = 170
    kOffsetCosManip = 171
    kOffsetCurveManip = 172
    kProjectionManip = 173
    kPolyProjectionManip = 174
    kProjectionUVManip = 175
    kProjectionMultiManip = 176
    kProjectTangentManip = 177
    kPropModManip = 178
    kQuadPtOnLineManip = 179
    kRbfSrfManip = 180
    kReverseCurveManip = 181
    kReverseCrvManip = 182
    kReverseSurfaceManip = 183
    kRevolveManip = 184
    kRevolvedPrimitiveManip = 185
    kSpotManip = 186
    kSpotCylinderManip = 187
    kTriplanarProjectionManip = 188
    kTrsManip = 189
    kDblTrsManip = 190
    kPivotManip2D = 191
    kManip2DContainer = 192
    kPolyMoveUVManip = 193
    kPolyMappingManip = 194
    kPolyModifierManip = 195
    kPolyMoveVertexManip = 196
    kPolyVertexNormalManip = 197
    kTexSmudgeUVManip = 198
    kTexLatticeDeformManip = 199
    kTexLattice = 200
    kTexSmoothManip = 201
    kTrsTransManip = 202
    kTrsInsertManip = 203
    kTrsXformManip = 204
    kManipulator2D = 205
    kTranslateManip2D = 206
    kPlanarProjectionManip = 207
    kPointOnCurveManip = 208
    kTowPointOnCurveManip = 209
    kMarkerManip = 210
    kPointOnLineManip = 211
    kPointOnSurfaceManip = 212
    kTranslateUVManip = 213
    kRotateBoxManip = 214
    kRotateManip = 215
    kHandleRotateManip = 216
    kRotateLimitsManip = 217
    kScaleLimitsManip = 218
    kScaleManip = 219
    kScalingBoxManip = 220
    kScriptManip = 221
    kSphericalProjectionManip = 222
    kTextureManip3D = 223
    kToggleManip = 224
    kTranslateBoxManip = 225
    kTranslateLimitsManip = 226
    kTranslateManip = 227
    kTrimManip = 228
    kJointTranslateManip = 229
    kManipulator = 230
    kCirclePointManip = 231
    kDimensionManip = 232
    kFixedLineManip = 233
    kLightProjectionGeometry = 234
    kLineArrowManip = 235
    kPointManip = 236
    kTriadManip = 237
    kNormalConstraint = 238
    kOrientConstraint = 239
    kPointConstraint = 240
    kSymmetryConstraint = 241
    kParentConstraint = 242
    kPoleVectorConstraint = 243
    kScaleConstraint = 244
    kTangentConstraint = 245
    kUnknownTransform = 246
    kWorld = 247
    kShape = 248
    kBaseLattice = 249
    kCamera = 250
    kCluster = 251
    kSoftMod = 252
    kCollision = 253
    kDummy = 254
    kEmitter = 255
    kField = 256
    kAir = 257
    kDrag = 258
    kGravity = 259
    kNewton = 260
    kRadial = 261
    kTurbulence = 262
    kUniform = 263
    kVortex = 264
    kGeometric = 265
    kCurve = 266
    kNurbsCurve = 267
    kNurbsCurveGeom = 268
    kDimension = 269
    kAngle = 270
    kAnnotation = 271
    kDistance = 272
    kArcLength = 273
    kRadius = 274
    kParamDimension = 275
    kDirectedDisc = 276
    kRenderRect = 277
    kEnvFogShape = 278
    kLattice = 279
    kLatticeGeom = 280
    kLocator = 281
    kDropoffLocator = 282
    kMarker = 283
    kOrientationMarker = 284
    kPositionMarker = 285
    kOrientationLocator = 286
    kTrimLocator = 287
    kPlane = 288
    kSketchPlane = 289
    kGroundPlane = 290
    kOrthoGrid = 291
    kSprite = 292
    kSurface = 293
    kNurbsSurface = 294
    kNurbsSurfaceGeom = 295
    kMesh = 296
    kMeshGeom = 297
    kRenderSphere = 298
    kFlexor = 299
    kClusterFlexor = 300
    kGuideLine = 301
    kLight = 302
    kAmbientLight = 303
    kNonAmbientLight = 304
    kAreaLight = 305
    kLinearLight = 306
    kNonExtendedLight = 307
    kDirectionalLight = 308
    kPointLight = 309
    kSpotLight = 310
    kParticle = 311
    kPolyToolFeedbackShape = 312
    kRigidConstraint = 313
    kRigid = 314
    kSpring = 315
    kUnknownDag = 316
    kDefaultLightList = 317
    kDeleteComponent = 318
    kDispatchCompute = 319
    kShadingEngine = 320
    kDisplacementShader = 321
    kDistanceBetween = 322
    kDOF = 323
    kDummyConnectable = 324
    kDynamicsController = 325
    kGeoConnectable = 326
    kExpression = 327
    kExtract = 328
    kFilter = 329
    kFilterClosestSample = 330
    kFilterEuler = 331
    kFilterSimplify = 332
    kGammaCorrect = 333
    kGeometryFilt = 334
    kBendLattice = 335
    kBlendShape = 336
    kCombinationShape = 337
    kBulgeLattice = 338
    kFFD = 339
    kFfdDualBase = 340
    kRigidDeform = 341
    kSculpt = 342
    kTextureDeformer = 343
    kTextureDeformerHandle = 344
    kTweak = 345
    kWeightGeometryFilt = 346
    kClusterFilter = 347
    kSoftModFilter = 348
    kJointCluster = 349
    kDeltaMush = 350
    kTension = 351
    kMorph = 352
    kSolidify = 353
    kProximityWrap = 354
    kWire = 355
    kGroupId = 356
    kGroupParts = 357
    kGuide = 358
    kHsvToRgb = 359
    kHyperGraphInfo = 360
    kHyperLayout = 361
    kHyperView = 362
    kIkSolver = 363
    kMCsolver = 364
    kPASolver = 365
    kSCsolver = 366
    kRPsolver = 367
    kSplineSolver = 368
    kIkSystem = 369
    kImagePlane = 370
    kLambert = 371
    kReflect = 372
    kBlinn = 373
    kPhong = 374
    kPhongExplorer = 375
    kLayeredShader = 376
    kStandardSurface = 377
    kLightInfo = 378
    kLeastSquares = 379
    kLightFogMaterial = 380
    kEnvFogMaterial = 381
    kLightList = 382
    kLightSource = 383
    kLuminance = 384
    kMakeGroup = 385
    kMaterial = 386
    kDiffuseMaterial = 387
    kLambertMaterial = 388
    kBlinnMaterial = 389
    kPhongMaterial = 390
    kLightSourceMaterial = 391
    kMaterialInfo = 392
    kMaterialTemplate = 393
    kMatrixAdd = 394
    kMatrixHold = 395
    kMatrixMult = 396
    kMatrixPass = 397
    kMatrixWtAdd = 398
    kMidModifier = 399
    kMidModifierWithMatrix = 400
    kPolyBevel = 401
    kPolyTweak = 402
    kPolyAppend = 403
    kPolyChipOff = 404
    kPolyCloseBorder = 405
    kPolyCollapseEdge = 406
    kPolyCollapseF = 407
    kPolyCylProj = 408
    kPolyDelEdge = 409
    kPolyDelFacet = 410
    kPolyDelVertex = 411
    kPolyExtrudeFacet = 412
    kPolyMapCut = 413
    kPolyMapDel = 414
    kPolyMapSew = 415
    kPolyMergeEdge = 416
    kPolyMergeFacet = 417
    kPolyMoveEdge = 418
    kPolyMoveFacet = 419
    kPolyMoveFacetUV = 420
    kPolyMoveUV = 421
    kPolyMoveVertex = 422
    kPolyMoveVertexUV = 423
    kPolyNormal = 424
    kPolyPlanProj = 425
    kPolyProj = 426
    kPolyQuad = 427
    kPolySmooth = 428
    kPolySoftEdge = 429
    kPolySphProj = 430
    kPolySplit = 431
    kPolySubdEdge = 432
    kPolySubdFacet = 433
    kPolyTriangulate = 434
    kPolyCreator = 435
    kPolyPrimitive = 436
    kPolyCone = 437
    kPolyCube = 438
    kPolyCylinder = 439
    kPolyMesh = 440
    kPolySphere = 441
    kPolyTorus = 442
    kPolyCreateFacet = 443
    kPolyUnite = 444
    kMotionPath = 445
    kPluginMotionPathNode = 446
    kMultilisterLight = 447
    kMultiplyDivide = 448
    kOldGeometryConstraint = 449
    kOpticalFX = 450
    kParticleAgeMapper = 451
    kParticleCloud = 452
    kParticleColorMapper = 453
    kParticleIncandecenceMapper = 454
    kParticleTransparencyMapper = 455
    kPartition = 456
    kPlace2dTexture = 457
    kPlace3dTexture = 458
    kPluginDependNode = 459
    kPluginLocatorNode = 460
    kPlusMinusAverage = 461
    kPointMatrixMult = 462
    kPolySeparate = 463
    kPostProcessList = 464
    kProjection = 465
    kRecord = 466
    kRenderUtilityList = 467
    kReverse = 468
    kRgbToHsv = 469
    kRigidSolver = 470
    kSet = 471
    kTextureBakeSet = 472
    kVertexBakeSet = 473
    kSetRange = 474
    kShaderGlow = 475
    kShaderList = 476
    kShadingMap = 477
    kSamplerInfo = 478
    kShapeFragment = 479
    kSimpleVolumeShader = 480
    kSl60 = 481
    kSnapshot = 482
    kStoryBoard = 483
    kSummaryObject = 484
    kSuper = 485
    kControl = 486
    kSurfaceLuminance = 487
    kSurfaceShader = 488
    kTextureList = 489
    kTextureEnv = 490
    kEnvBall = 491
    kEnvCube = 492
    kEnvChrome = 493
    kEnvSky = 494
    kEnvSphere = 495
    kTexture2d = 496
    kBulge = 497
    kChecker = 498
    kCloth = 499
    kFileTexture = 500
    kFractal = 501
    kGrid = 502
    kMountain = 503
    kRamp = 504
    kStencil = 505
    kWater = 506
    kTexture3d = 507
    kBrownian = 508
    kCloud = 509
    kCrater = 510
    kGranite = 511
    kLeather = 512
    kMarble = 513
    kRock = 514
    kSnow = 515
    kSolidFractal = 516
    kStucco = 517
    kTxSl = 518
    kWood = 519
    kTime = 520
    kTimeToUnitConversion = 521
    kRenderSetup = 522
    kRenderGlobals = 523
    kRenderGlobalsList = 524
    kRenderQuality = 525
    kResolution = 526
    kHardwareRenderGlobals = 527
    kArrayMapper = 528
    kUnitConversion = 529
    kUnitToTimeConversion = 530
    kUseBackground = 531
    kUnknown = 532
    kVectorProduct = 533
    kVolumeShader = 534
    kComponent = 535
    kCurveCVComponent = 536
    kCurveEPComponent = 537
    kCurveKnotComponent = 538
    kCurveParamComponent = 539
    kIsoparmComponent = 540
    kPivotComponent = 541
    kSurfaceCVComponent = 542
    kSurfaceEPComponent = 543
    kSurfaceKnotComponent = 544
    kEdgeComponent = 545
    kLatticeComponent = 546
    kSurfaceRangeComponent = 547
    kDecayRegionCapComponent = 548
    kDecayRegionComponent = 549
    kMeshComponent = 550
    kMeshEdgeComponent = 551
    kMeshPolygonComponent = 552
    kMeshFrEdgeComponent = 553
    kMeshVertComponent = 554
    kMeshFaceVertComponent = 555
    kOrientationComponent = 556
    kSubVertexComponent = 557
    kMultiSubVertexComponent = 558
    kSetGroupComponent = 559
    kDynParticleSetComponent = 560
    kSelectionItem = 561
    kDagSelectionItem = 562
    kNonDagSelectionItem = 563
    kItemList = 564
    kAttribute = 565
    kNumericAttribute = 566
    kDoubleAngleAttribute = 567
    kFloatAngleAttribute = 568
    kDoubleLinearAttribute = 569
    kFloatLinearAttribute = 570
    kTimeAttribute = 571
    kEnumAttribute = 572
    kUnitAttribute = 573
    kTypedAttribute = 574
    kCompoundAttribute = 575
    kGenericAttribute = 576
    kLightDataAttribute = 577
    kMatrixAttribute = 578
    kFloatMatrixAttribute = 579
    kMessageAttribute = 580
    kOpaqueAttribute = 581
    kPlugin = 582
    kData = 583
    kComponentListData = 584
    kDoubleArrayData = 585
    kIntArrayData = 586
    kUintArrayData = 587
    kLatticeData = 588
    kMatrixData = 589
    kMeshData = 590
    kNurbsSurfaceData = 591
    kNurbsCurveData = 592
    kNumericData = 593
    kData2Double = 594
    kData2Float = 595
    kData2Int = 596
    kData2Long = 596
    kData2Short = 597
    kData3Double = 598
    kData3Float = 599
    kData3Int = 600
    kData3Long = 600
    kData3Short = 601
    kPluginData = 602
    kPointArrayData = 603
    kMatrixArrayData = 604
    kSphereData = 605
    kStringData = 606
    kStringArrayData = 607
    kVectorArrayData = 608
    kSelectionList = 609
    kTransformGeometry = 610
    kCommEdgePtManip = 611
    kCommEdgeOperManip = 612
    kCommEdgeSegmentManip = 613
    kCommCornerManip = 614
    kCommCornerOperManip = 615
    kPluginDeformerNode = 616
    kTorus = 617
    kPolyBoolOp = 618
    kSingleShadingSwitch = 619
    kDoubleShadingSwitch = 620
    kTripleShadingSwitch = 621
    kNurbsSquare = 622
    kAnisotropy = 623
    kNonLinear = 624
    kDeformFunc = 625
    kDeformBend = 626
    kDeformTwist = 627
    kDeformSquash = 628
    kDeformFlare = 629
    kDeformSine = 630
    kDeformWave = 631
    kDeformBendManip = 632
    kDeformTwistManip = 633
    kDeformSquashManip = 634
    kDeformFlareManip = 635
    kDeformSineManip = 636
    kDeformWaveManip = 637
    kSoftModManip = 638
    kDistanceManip = 639
    kScript = 640
    kCurveFromMeshEdge = 641
    kCurveCurveIntersect = 642
    kNurbsCircular3PtArc = 643
    kNurbsCircular2PtArc = 644
    kOffsetSurface = 645
    kRoundConstantRadius = 646
    kRoundRadiusManip = 647
    kRoundRadiusCrvManip = 648
    kRoundConstantRadiusManip = 649
    kThreePointArcManip = 650
    kTwoPointArcManip = 651
    kTextButtonManip = 652
    kOffsetSurfaceManip = 653
    kImageData = 654
    kImageLoad = 655
    kImageSave = 656
    kImageNetSrc = 657
    kImageNetDest = 658
    kImageRender = 659
    kImageAdd = 660
    kImageDiff = 661
    kImageMultiply = 662
    kImageOver = 663
    kImageUnder = 664
    kImageColorCorrect = 665
    kImageBlur = 666
    kImageFilter = 667
    kImageDepth = 668
    kImageDisplay = 669
    kImageView = 670
    kImageMotionBlur = 671
    kViewColorManager = 672
    kMatrixFloatData = 673
    kSkinShader = 674
    kComponentManip = 675
    kSelectionListData = 676
    kObjectFilter = 677
    kObjectMultiFilter = 678
    kObjectNameFilter = 679
    kObjectTypeFilter = 680
    kObjectAttrFilter = 681
    kObjectRenderFilter = 682
    kObjectScriptFilter = 683
    kSelectionListOperator = 684
    kSubdiv = 685
    kPolyToSubdiv = 686
    kSkinClusterFilter = 687
    kKeyingGroup = 688
    kCharacter = 689
    kCharacterOffset = 690
    kDagPose = 691
    kStitchAsNurbsShell = 692
    kExplodeNurbsShell = 693
    kNurbsBoolean = 694
    kStitchSrfManip = 695
    kForceUpdateManip = 696
    kPluginManipContainer = 697
    kPolySewEdge = 698
    kPolyMergeVert = 699
    kPolySmoothFacet = 700
    kSmoothCurve = 701
    kGlobalStitch = 702
    kSubdivCVComponent = 703
    kSubdivEdgeComponent = 704
    kSubdivFaceComponent = 705
    kUVManip2D = 706
    kTranslateUVManip2D = 707
    kRotateUVManip2D = 708
    kScaleUVManip2D = 709
    kPolyTweakUV = 710
    kMoveUVShellManip2D = 711
    kPluginShape = 712
    kGeometryData = 713
    kSingleIndexedComponent = 714
    kDoubleIndexedComponent = 715
    kTripleIndexedComponent = 716
    kExtendSurfaceDistanceManip = 717
    kSquareSrf = 718
    kSquareSrfManip = 719
    kSubdivToPoly = 720
    kDynBase = 721
    kDynEmitterManip = 722
    kDynFieldsManip = 723
    kDynBaseFieldManip = 724
    kDynAirManip = 725
    kDynNewtonManip = 726
    kDynTurbulenceManip = 727
    kDynSpreadManip = 728
    kDynAttenuationManip = 729
    kDynArrayAttrsData = 730
    kPluginFieldNode = 731
    kPluginEmitterNode = 732
    kPluginSpringNode = 733
    kDisplayLayer = 734
    kDisplayLayerManager = 735
    kPolyColorPerVertex = 736
    kCreateColorSet = 737
    kDeleteColorSet = 738
    kCopyColorSet = 739
    kBlendColorSet = 740
    kPolyColorMod = 741
    kPolyColorDel = 742
    kCharacterMappingData = 743
    kDynSweptGeometryData = 744
    kWrapFilter = 745
    kMeshVtxFaceComponent = 746
    kBinaryData = 747
    kAttribute2Double = 748
    kAttribute2Float = 749
    kAttribute2Short = 750
    kAttribute2Int = 751
    kAttribute2Long = 751
    kAttribute3Double = 752
    kAttribute3Float = 753
    kAttribute3Short = 754
    kAttribute3Int = 755
    kAttribute3Long = 755
    kReference = 756
    kBlindData = 757
    kBlindDataTemplate = 758
    kPolyBlindData = 759
    kPolyNormalPerVertex = 760
    kNurbsToSubdiv = 761
    kPluginIkSolver = 762
    kInstancer = 763
    kMoveVertexManip = 764
    kStroke = 765
    kBrush = 766
    kStrokeGlobals = 767
    kPluginGeometryData = 768
    kLightLink = 769
    kDynGlobals = 770
    kPolyReduce = 771
    kLodThresholds = 772
    kChooser = 773
    kLodGroup = 774
    kMultDoubleLinear = 775
    kFourByFourMatrix = 776
    kTowPointOnSurfaceManip = 777
    kSurfaceEdManip = 778
    kSurfaceFaceComponent = 779
    kClipScheduler = 780
    kClipLibrary = 781
    kSubSurface = 782
    kSmoothTangentSrf = 783
    kRenderPass = 784
    kRenderPassSet = 785
    kRenderLayer = 786
    kRenderLayerManager = 787
    kPassContributionMap = 788
    kPrecompExport = 789
    kRenderTarget = 790
    kRenderedImageSource = 791
    kImageSource = 792
    kPolyFlipEdge = 793
    kPolyExtrudeEdge = 794
    kAnimBlend = 795
    kAnimBlendInOut = 796
    kPolyAppendVertex = 797
    kUvChooser = 798
    kSubdivCompId = 799
    kVolumeAxis = 800
    kDeleteUVSet = 801
    kSubdHierBlind = 802
    kSubdBlindData = 803
    kCharacterMap = 804
    kLayeredTexture = 805
    kSubdivCollapse = 806
    kParticleSamplerInfo = 807
    kCopyUVSet = 808
    kCreateUVSet = 809
    kClip = 810
    kPolySplitVert = 811
    kSubdivData = 812
    kSubdivGeom = 813
    kUInt64ArrayData = 814
    kInt64ArrayData = 815
    kPolySplitEdge = 816
    kSubdivReverseFaces = 817
    kMeshMapComponent = 818
    kSectionManip = 819
    kXsectionSubdivEdit = 820
    kSubdivToNurbs = 821
    kEditCurve = 822
    kEditCurveManip = 823
    kCrossSectionManager = 824
    kCreateSectionManip = 825
    kCrossSectionEditManip = 826
    kDropOffFunction = 827
    kSubdBoolean = 828
    kSubdModifyEdge = 829
    kModifyEdgeCrvManip = 830
    kModifyEdgeManip = 831
    kScalePointManip = 832
    kTransformBoxManip = 833
    kSymmetryLocator = 834
    kSymmetryMapVector = 835
    kSymmetryMapCurve = 836
    kCurveFromSubdivEdge = 837
    kCreateBPManip = 838
    kModifyEdgeBaseManip = 839
    kSubdExtrudeFace = 840
    kSubdivSurfaceVarGroup = 841
    kSfRevolveManip = 842
    kCurveFromSubdivFace = 843
    kUnused1 = 844
    kUnused2 = 845
    kUnused3 = 846
    kUnused4 = 847
    kUnused5 = 848
    kUnused6 = 849
    kPolyTransfer = 850
    kPolyAverageVertex = 851
    kPolyAutoProj = 852
    kPolyLayoutUV = 853
    kPolyMapSewMove = 854
    kSubdModifier = 855
    kSubdMoveVertex = 856
    kSubdMoveEdge = 857
    kSubdMoveFace = 858
    kSubdDelFace = 859
    kSnapshotShape = 860
    kSubdivMapComponent = 861
    kJiggleDeformer = 862
    kGlobalCacheControls = 863
    kDiskCache = 864
    kSubdCloseBorder = 865
    kSubdMergeVert = 866
    kBoxData = 867
    kBox = 868
    kRenderBox = 869
    kSubdSplitFace = 870
    kVolumeFog = 871
    kSubdTweakUV = 872
    kSubdMapCut = 873
    kSubdLayoutUV = 874
    kSubdMapSewMove = 875
    kOcean = 876
    kVolumeNoise = 877
    kSubdAutoProj = 878
    kSubdSubdivideFace = 879
    kNoise = 880
    kAttribute4Double = 881
    kData4Double = 882
    kSubdPlanProj = 883
    kSubdTweak = 884
    kSubdProjectionManip = 885
    kSubdMappingManip = 886
    kHardwareReflectionMap = 887
    kPolyNormalizeUV = 888
    kPolyFlipUV = 889
    kHwShaderNode = 890
    kPluginHardwareShader = 891
    kPluginHwShaderNode = 892
    kSubdAddTopology = 893
    kSubdCleanTopology = 894
    kImplicitCone = 895
    kImplicitSphere = 896
    kRampShader = 897
    kVolumeLight = 898
    kOceanShader = 899
    kBevelPlus = 900
    kStyleCurve = 901
    kPolyCut = 902
    kPolyPoke = 903
    kPolyWedgeFace = 904
    kPolyCutManipContainer = 905
    kPolyCutManip = 906
    kPolyMirrorManipContainer = 907
    kPolyPokeManip = 908
    kFluidTexture3D = 909
    kFluidTexture2D = 910
    kPolyMergeUV = 911
    kPolyStraightenUVBorder = 912
    kAlignManip = 913
    kPluginTransformNode = 914
    kFluid = 915
    kFluidGeom = 916
    kFluidData = 917
    kSmear = 918
    kStringShadingSwitch = 919
    kStudioClearCoat = 920
    kFluidEmitter = 921
    kHeightField = 922
    kGeoConnector = 923
    kSnapshotPath = 924
    kPluginObjectSet = 925
    kQuadShadingSwitch = 926
    kPolyExtrudeVertex = 927
    kPairBlend = 928
    kTextManip = 929
    kViewManip = 930
    kXformManip = 931
    kMute = 932
    kConstraint = 933
    kTrimWithBoundaries = 934
    kCurveFromMeshCoM = 935
    kFollicle = 936
    kHairSystem = 937
    kRemapValue = 938
    kRemapColor = 939
    kRemapHsv = 940
    kHairConstraint = 941
    kTimeFunction = 942
    kMentalRayTexture = 943
    kObjectBinFilter = 944
    kPolySmoothProxy = 945
    kPfxGeometry = 946
    kPfxHair = 947
    kHairTubeShader = 948
    kPsdFileTexture = 949
    kKeyframeDelta = 950
    kKeyframeDeltaMove = 951
    kKeyframeDeltaScale = 952
    kKeyframeDeltaAddRemove = 953
    kKeyframeDeltaBlockAddRemove = 954
    kKeyframeDeltaInfType = 955
    kKeyframeDeltaTangent = 956
    kKeyframeDeltaWeighted = 957
    kKeyframeDeltaBreakdown = 958
    kPolyMirror = 959
    kPolyCreaseEdge = 960
    kPolyPinUV = 961
    kHikEffector = 962
    kHikIKEffector = 963
    kHikFKJoint = 964
    kHikSolver = 965
    kHikHandle = 966
    kProxyManager = 967
    kPolyAutoProjManip = 968
    kPolyPrism = 969
    kPolyPyramid = 970
    kPolySplitRing = 971
    kPfxToon = 972
    kToonLineAttributes = 973
    kPolyDuplicateEdge = 974
    kFacade = 975
    kMaterialFacade = 976
    kEnvFacade = 977
    kAISEnvFacade = 978
    kLineModifier = 979
    kPolyArrow = 980
    kPolyPrimitiveMisc = 981
    kPolyPlatonicSolid = 982
    kPolyPipe = 983
    kHikFloorContactMarker = 984
    kHikGroundPlane = 985
    kPolyComponentData = 986
    kPolyHelix = 987
    kCacheFile = 988
    kHistorySwitch = 989
    kClosestPointOnMesh = 990
    kUVPin = 991
    kProximityPin = 992
    kTransferAttributes = 993
    kDynamicConstraint = 994
    kNComponent = 995
    kPolyBridgeEdge = 996
    kCacheableNode = 997
    kNucleus = 998
    kNBase = 999
    kCacheBase = 1000
    kCacheBlend = 1001
    kCacheTrack = 1002
    kKeyframeRegionManip = 1003
    kCurveNormalizerAngle = 1004
    kCurveNormalizerLinear = 1005
    kHyperLayoutDG = 1006
    kPluginImagePlaneNode = 1007
    kNCloth = 1008
    kNParticle = 1009
    kNRigid = 1010
    kPluginParticleAttributeMapperNode = 1011
    kCameraSet = 1012
    kPluginCameraSet = 1013
    kContainer = 1014
    kFloatVectorArrayData = 1015
    kNObjectData = 1016
    kNObject = 1017
    kPluginConstraintNode = 1018
    kAsset = 1019
    kPolyEdgeToCurve = 1020
    kAnimLayer = 1021
    kBlendNodeBase = 1022
    kBlendNodeBoolean = 1023
    kBlendNodeDouble = 1024
    kBlendNodeDoubleAngle = 1025
    kBlendNodeDoubleLinear = 1026
    kBlendNodeEnum = 1027
    kBlendNodeFloat = 1028
    kBlendNodeFloatAngle = 1029
    kBlendNodeFloatLinear = 1030
    kBlendNodeInt16 = 1031
    kBlendNodeInt32 = 1032
    kBlendNodeAdditiveScale = 1033
    kBlendNodeAdditiveRotation = 1034
    kPluginManipulatorNode = 1035
    kNIdData = 1036
    kNId = 1037
    kFloatArrayData = 1038
    kMembrane = 1039
    kMergeVertsToolManip = 1040
    kUint64SingleIndexedComponent = 1041
    kPolyToolFeedbackManip = 1042
    kPolySelectEditFeedbackManip = 1043
    kWriteToFrameBuffer = 1044
    kWriteToColorBuffer = 1045
    kWriteToVectorBuffer = 1046
    kWriteToDepthBuffer = 1047
    kWriteToLabelBuffer = 1048
    kStereoCameraMaster = 1049
    kSequenceManager = 1050
    kSequencer = 1051
    kShot = 1052
    kBlendNodeTime = 1053
    kCreateBezierManip = 1054
    kBezierCurve = 1055
    kBezierCurveData = 1056
    kNurbsCurveToBezier = 1057
    kBezierCurveToNurbs = 1058
    kPolySpinEdge = 1059
    kPolyHoleFace = 1060
    kPointOnPolyConstraint = 1061
    kPolyConnectComponents = 1062
    kSkinBinding = 1063
    kVolumeBindManip = 1064
    kVertexWeightSet = 1065
    kNearestPointOnCurve = 1066
    kColorProfile = 1067
    kAdskMaterial = 1068
    kContainerBase = 1069
    kDagContainer = 1070
    kPolyUVRectangle = 1071
    kHardwareRenderingGlobals = 1072
    kPolyProjectCurve = 1073
    kRenderingList = 1074
    kPolyExtrudeManip = 1075
    kPolyExtrudeManipContainer = 1076
    kThreadedDevice = 1077
    kClientDevice = 1078
    kPluginClientDevice = 1079
    kPluginThreadedDevice = 1080
    kTimeWarp = 1081
    kAssembly = 1082
    kClipGhostShape = 1083
    kClipToGhostData = 1084
    kMandelbrot = 1085
    kMandelbrot3D = 1086
    kGreasePlane = 1087
    kGreasePlaneRenderShape = 1088
    kGreasePencilSequence = 1089
    kEditMetadata = 1090
    kCreaseSet = 1091
    kPolyEditEdgeFlow = 1092
    kFosterParent = 1093
    kSnapUVManip2D = 1094
    kToolContext = 1095
    kNLE = 1096
    kShrinkWrapFilter = 1097
    kEditsManager = 1098
    kPolyBevel2 = 1099
    kPolyCBoolOp = 1100
    kGeomBind = 1101
    kColorMgtGlobals = 1102
    kPolyBevel3 = 1103
    kTimeEditorClipBase = 1104
    kTimeEditorClipEvaluator = 1105
    kTimeEditorClip = 1106
    kTimeEditor = 1107
    kTimeEditorTracks = 1108
    kTimeEditorInterpolator = 1109
    kTimeEditorAnimSource = 1110
    kCaddyManipBase = 1111
    kPolyCaddyManip = 1112
    kPolyModifierManipContainer = 1113
    kPolyRemesh = 1114
    kPolyContourProj = 1115
    kContourProjectionManip = 1116
    kNodeGraphEditorInfo = 1117
    kNodeGraphEditorBookmarks = 1118
    kNodeGraphEditorBookmarkInfo = 1119
    kPluginSkinCluster = 1120
    kPluginGeometryFilter = 1121
    kPluginBlendShape = 1122
    kPolyPassThru = 1123
    kTrackInfoManager = 1124
    kPolyClean = 1125
    kShapeEditorManager = 1126
    kOceanDeformer = 1127
    kPoseInterpolatorManager = 1128
    kControllerTag = 1129
    kReForm = 1130
    kCustomEvaluatorClusterNode = 1131
    kPolyCircularize = 1132
    kArubaTesselate = 1133
    kReorderUVSet = 1134
    kUfeProxyTransform = 1135
    kDecomposeMatrix = 1136
    kComposeMatrix = 1137
    kBlendMatrix = 1138
    kPickMatrix = 1139
    kAimMatrix = 1140
    kPrimitiveFalloff = 1141
    kBlendFalloff = 1142
    kUniformFalloff = 1143
    kTransferFalloff = 1144
    kComponentFalloff = 1145
    kProximityFalloff = 1146
    kSubsetFalloff = 1147
    kWeightFunctionData = 1148
    kFalloffEval = 1149
    kComponentMatch = 1150
    kPolyUnsmooth = 1151
    kPolySmartExtrude = 1152
    kPolySmartExtrudeManip = 1153
    kPolyReFormManipContainer = 1154
    kPolyReFormManip = 1155
    kPolyAxis = 1156
    kAngleToDoubleNode = 1157
    kDoubleToAngleNode = 1158
    kAbsolute = 1159
    kACos = 1160
    kAnd = 1161
    kASin = 1162
    kATan = 1163
    kATan2 = 1164
    kAverage = 1165
    kCeil = 1166
    kClampRange = 1167
    kCos = 1168
    kDeterminant = 1169
    kEqual = 1170
    kFloor = 1171
    kGreaterThan = 1172
    kInverseLinearInterpolation = 1173
    kLength = 1174
    kLessThan = 1175
    kLinearInterpolation = 1176
    kLog = 1177
    kMax = 1178
    kMin = 1179
    kModulo = 1180
    kMultiply = 1181
    kNegate = 1182
    kNormalize = 1183
    kNot = 1184
    kOr = 1185
    kPIConstant = 1186
    kPower = 1187
    kRotateVector = 1188
    kRound = 1189
    kSin = 1190
    kSmoothStep = 1191
    kSum = 1192
    kTan = 1193
    kTruncate = 1194
    kDotProduct = 1195
    kCrossProduct = 1196
    kMultiplyPointByMatrix = 1197
    kMultiplyVectorByMatrix = 1198
    kAxisFromMatrix = 1199
    kDivide = 1200
    kSubtract = 1201
    kTranslationFromMatrix = 1202
    kRowFromMatrix = 1203
    kColumnFromMatrix = 1204
    kScaleFromMatrix = 1205
    kRotationFromMatrix = 1206
    kParentMatrix = 1207
    kPolyBevelCutback = 1208
    kOpenPBRSurface = 1209
    kShotLabel = 1211

# Every node type's kind in the reference (pymel's 2025 cache; the 2027 DL math nodes share their plain twin's kind, as
# their 2027 pages say: addDL and multDL are kAddDoubleLinear and kMultDoubleLinear).
_REF_FN_OF_TYPE = {
    "addDL": MFn.kAddDoubleLinear, "multDL": MFn.kMultDoubleLinear,
    "AISEnvFacade": MFn.kAISEnvFacade, "THarrayMapper": MFn.kPluginParticleAttributeMapperNode,
    "THassembly": MFn.kAssembly, "THblendShape": MFn.kPluginBlendShape, "THcameraSet": MFn.kPluginCameraSet,
    "THclientDevice": MFn.kPluginClientDevice, "THconstraint": MFn.kPluginConstraintNode,
    "THcustomTransform": MFn.kPluginTransformNode, "THdeformer": MFn.kPluginDeformerNode,
    "THdependNode": MFn.kPluginDependNode, "THdynEmitter": MFn.kPluginEmitterNode, "THdynField": MFn.kPluginFieldNode,
    "THdynSpring": MFn.kPluginSpringNode, "THfluidEmitter": MFn.kPluginEmitterNode,
    "THgeometryFilter": MFn.kPluginGeometryFilter, "THhardwareShader": MFn.kPluginHardwareShader,
    "THhwShader": MFn.kPluginHwShaderNode, "THikSolverNode": MFn.kPluginIkSolver,
    "THimagePlane": MFn.kPluginImagePlaneNode, "THlocatorShape": MFn.kPluginLocatorNode,
    "THmanip": MFn.kPluginManipulatorNode, "THmanipContainer": MFn.kPluginManipContainer,
    "THmotionPath": MFn.kPluginMotionPathNode, "THobjectSet": MFn.kPluginObjectSet,
    "THskinCluster": MFn.kPluginSkinCluster, "THsurfaceShape": MFn.kPluginShape,
    "THthreadedDevice": MFn.kPluginThreadedDevice, "TadskAssetInstanceNode_TdependNode": MFn.kDependencyNode,
    "TadskAssetInstanceNode_TdnTx2D": MFn.kTexture2d, "TadskAssetInstanceNode_TlightShape": MFn.kLight,
    "aboutToSetValueTestNode": MFn.kDependencyNode, "absolute": MFn.kAbsolute, "absoluteDL": MFn.kAbsolute,
    "abstractBaseCreate": MFn.kCreate, "abstractBaseNurbsConversion": MFn.kCreate, "acos": MFn.kACos,
    "acosDL": MFn.kACos, "addDoubleLinear": MFn.kAddDoubleLinear, "addMatrix": MFn.kMatrixAdd,
    "adskMaterial": MFn.kAdskMaterial, "aimConstraint": MFn.kAimConstraint, "aimMatrix": MFn.kAimMatrix,
    "airField": MFn.kAir, "alignCurve": MFn.kAlignCurve, "alignSurface": MFn.kAlignSurface,
    "ambientLight": MFn.kAmbientLight, "and": MFn.kAnd, "angleBetween": MFn.kAngleBetween,
    "angleBetweenDL": MFn.kAngleBetween, "angleDimension": MFn.kAngle, "animBlend": MFn.kAnimBlend,
    "animBlendInOut": MFn.kAnimBlendInOut, "animBlendNodeAdditive": MFn.kBlendNodeDouble,
    "animBlendNodeAdditiveDA": MFn.kBlendNodeDoubleAngle, "animBlendNodeAdditiveDL": MFn.kBlendNodeDoubleLinear,
    "animBlendNodeAdditiveF": MFn.kBlendNodeFloat, "animBlendNodeAdditiveFA": MFn.kBlendNodeFloatAngle,
    "animBlendNodeAdditiveFL": MFn.kBlendNodeFloatLinear, "animBlendNodeAdditiveI16": MFn.kBlendNodeInt16,
    "animBlendNodeAdditiveI32": MFn.kBlendNodeInt32, "animBlendNodeAdditiveRotation": MFn.kBlendNodeAdditiveRotation,
    "animBlendNodeAdditiveScale": MFn.kBlendNodeAdditiveScale, "animBlendNodeBase": MFn.kDependencyNode,
    "animBlendNodeBoolean": MFn.kBlendNodeBoolean, "animBlendNodeEnum": MFn.kBlendNodeEnum,
    "animBlendNodeTime": MFn.kBlendNodeTime, "animClip": MFn.kClip, "animCurve": MFn.kAnimCurve,
    "animCurveTA": MFn.kAnimCurveTimeToAngular, "animCurveTL": MFn.kAnimCurveTimeToDistance,
    "animCurveTT": MFn.kAnimCurveTimeToTime, "animCurveTU": MFn.kAnimCurveTimeToUnitless,
    "animCurveUA": MFn.kAnimCurveUnitlessToAngular, "animCurveUL": MFn.kAnimCurveUnitlessToDistance,
    "animCurveUT": MFn.kAnimCurveUnitlessToTime, "animCurveUU": MFn.kAnimCurveUnitlessToUnitless,
    "animLayer": MFn.kAnimLayer, "anisotropic": MFn.kAnisotropy, "annotationShape": MFn.kAnnotation,
    "arcLengthDimension": MFn.kArcLength, "areaLight": MFn.kAreaLight, "arrayMapper": MFn.kArrayMapper,
    "arubaTessellate": MFn.kArubaTesselate, "asin": MFn.kASin, "asinDL": MFn.kASin, "assembly": MFn.kAssembly,
    "assemblyDefinition": MFn.kAssembly, "assemblyReference": MFn.kAssembly, "atan": MFn.kATan, "atan2": MFn.kATan2,
    "atan2DL": MFn.kATan2, "atanDL": MFn.kATan, "attachCurve": MFn.kAttachCurve, "attachSurface": MFn.kAttachSurface,
    "attrHierarchyTest": MFn.kDependencyNode, "audio": MFn.kAudio, "average": MFn.kAverage, "averageDL": MFn.kAverage,
    "avgCurves": MFn.kAvgCurves, "avgNurbsSurfacePoints": MFn.kAvgNurbsSurfacePoints,
    "avgSurfacePoints": MFn.kAvgSurfacePoints, "axisFromMatrix": MFn.kAxisFromMatrix,
    "axisFromMatrixDL": MFn.kAxisFromMatrix, "bakeSet": MFn.kSet, "baseGeometryVarGroup": MFn.kTransform,
    "baseLattice": MFn.kBaseLattice, "baseShadingSwitch": MFn.kDependencyNode, "bevel": MFn.kBevel,
    "bevelPlus": MFn.kBevelPlus, "bezierCurve": MFn.kNurbsCurve, "bezierCurveToNurbs": MFn.kBezierCurveToNurbs,
    "birailSrf": MFn.kBirailSrf, "blend": MFn.kBlend, "blendColorSets": MFn.kBlendColorSet,
    "blendColors": MFn.kBlendColors, "blendDevice": MFn.kBlendDevice, "blendFalloff": MFn.kBlendFalloff,
    "blendMatrix": MFn.kBlendMatrix, "blendShape": MFn.kBlendShape, "blendTwoAttr": MFn.kBlendTwoAttr,
    "blendWeighted": MFn.kBlendWeighted, "blindDataTemplate": MFn.kBlindDataTemplate, "blinn": MFn.kBlinn,
    "boneLattice": MFn.kBulgeLattice, "boolean": MFn.kNurbsBoolean, "boundary": MFn.kBoundary,
    "boundaryBase": MFn.kCreate, "brownian": MFn.kBrownian, "brush": MFn.kBrush, "bulge": MFn.kBulge,
    "bump2d": MFn.kBump, "bump3d": MFn.kBump3d, "cacheBase": MFn.kCacheBase, "cacheBlend": MFn.kCacheTrack,
    "cacheFile": MFn.kCacheFile, "camera": MFn.kCamera, "cameraSet": MFn.kCameraSet, "cameraView": MFn.kCameraView,
    "ceil": MFn.kCeil, "ceilDL": MFn.kCeil, "character": MFn.kCharacter, "characterMap": MFn.kCharacterMap,
    "characterOffset": MFn.kCharacterOffset, "checker": MFn.kChecker, "choice": MFn.kChoice, "chooser": MFn.kChooser,
    "clamp": MFn.kClampColor, "clampRange": MFn.kClampRange, "clampRangeDL": MFn.kClampRange,
    "clientDevice": MFn.kClientDevice, "clipGhostShape": MFn.kClipGhostShape, "clipLibrary": MFn.kClipLibrary,
    "clipScheduler": MFn.kClipScheduler, "clipToGhostData": MFn.kClipToGhostData, "closeCurve": MFn.kCloseCurve,
    "closeSurface": MFn.kCloseSurface, "closestPointOnMesh": MFn.kClosestPointOnMesh,
    "closestPointOnSurface": MFn.kClosestPointOnSurface, "cloth": MFn.kCloth, "cloud": MFn.kCloud,
    "cluster": MFn.kClusterFilter, "clusterFlexorShape": MFn.kClusterFlexor, "clusterHandle": MFn.kCluster,
    "collisionModel": MFn.kCollision, "colorManagementGlobals": MFn.kColorMgtGlobals,
    "colorProfile": MFn.kColorProfile, "columnFromMatrix": MFn.kColumnFromMatrix,
    "columnFromMatrixDL": MFn.kColumnFromMatrix, "combinationShape": MFn.kCombinationShape,
    "compactPlugArrayTest": MFn.kDependencyNode, "componentFalloff": MFn.kComponentFalloff,
    "componentMatch": MFn.kComponentMatch, "componentTagBase": MFn.kDependencyNode,
    "composeMatrix": MFn.kComposeMatrix, "condition": MFn.kCondition, "constraint": MFn.kConstraint,
    "container": MFn.kContainer, "containerBase": MFn.kContainerBase, "contrast": MFn.kContrast,
    "controlPoint": MFn.kShape, "controller": MFn.kControllerTag, "copyColorSet": MFn.kCopyColorSet,
    "copyUVSet": MFn.kCopyUVSet, "cos": MFn.kCos, "cosDL": MFn.kCos, "crater": MFn.kCrater,
    "creaseSet": MFn.kCreaseSet, "createColorSet": MFn.kCreateColorSet, "createUVSet": MFn.kCreateUVSet,
    "crossProduct": MFn.kCrossProduct, "crossProductDL": MFn.kCrossProduct, "curveFromMesh": MFn.kDependencyNode,
    "curveFromMeshCoM": MFn.kCurveFromMeshCoM, "curveFromMeshEdge": MFn.kCurveFromMeshEdge,
    "curveFromSubdiv": MFn.kCreate, "curveFromSubdivEdge": MFn.kCurveFromSubdivEdge,
    "curveFromSubdivFace": MFn.kCurveFromSubdivFace, "curveFromSurface": MFn.kCurveFromSurface,
    "curveFromSurfaceBnd": MFn.kCurveFromSurfaceBnd, "curveFromSurfaceCoS": MFn.kCurveFromSurfaceCoS,
    "curveFromSurfaceIso": MFn.kCurveFromSurfaceIso, "curveInfo": MFn.kCurveInfo,
    "curveIntersect": MFn.kCurveCurveIntersect, "curveNormalizer": MFn.kDependencyNode,
    "curveNormalizerAngle": MFn.kCurveNormalizerAngle, "curveNormalizerLinear": MFn.kCurveNormalizerLinear,
    "curveRange": MFn.kCreate, "curveShape": MFn.kCurve, "curveVarGroup": MFn.kCurveVarGroup,
    "dagContainer": MFn.kDagContainer, "dagNode": MFn.kDagNode, "dagPose": MFn.kDagPose,
    "dataBlockTest": MFn.kDependencyNode, "decomposeMatrix": MFn.kDecomposeMatrix,
    "defaultLightList": MFn.kDefaultLightList, "defaultRenderUtilityList": MFn.kRenderUtilityList,
    "defaultRenderingList": MFn.kRenderingList, "defaultShaderList": MFn.kShaderList,
    "defaultTextureList": MFn.kTextureList, "deformBend": MFn.kDeformBend, "deformFlare": MFn.kDeformFlare,
    "deformFunc": MFn.kDeformFunc, "deformSine": MFn.kDeformSine, "deformSquash": MFn.kDeformSquash,
    "deformTwist": MFn.kDeformTwist, "deformWave": MFn.kDeformWave, "deformableShape": MFn.kShape,
    "deleteColorSet": MFn.kDeleteColorSet, "deleteComponent": MFn.kDeleteComponent, "deleteUVSet": MFn.kDeleteUVSet,
    "deltaMush": MFn.kDeltaMush, "dependNode": MFn.kDependencyNode, "detachCurve": MFn.kDetachCurve,
    "detachSurface": MFn.kDetachSurface, "determinant": MFn.kDeterminant, "determinantDL": MFn.kDeterminant,
    "dimensionShape": MFn.kDimension, "directedDisc": MFn.kDirectedDisc, "directionalLight": MFn.kDirectionalLight,
    "diskCache": MFn.kDiskCache, "displacementShader": MFn.kDisplacementShader, "displayLayer": MFn.kDisplayLayer,
    "displayLayerManager": MFn.kDisplayLayerManager, "distanceBetween": MFn.kDistanceBetween,
    "distanceBetweenDL": MFn.kDistanceBetween, "distanceDimShape": MFn.kDistance, "divide": MFn.kDivide,
    "divideDL": MFn.kDivide, "dof": MFn.kDOF, "dotProduct": MFn.kDotProduct, "dotProductDL": MFn.kDotProduct,
    "doubleShadingSwitch": MFn.kDoubleShadingSwitch, "dpBirailSrf": MFn.kDPbirailSrf, "dragField": MFn.kDrag,
    "dropoffLocator": MFn.kDropoffLocator, "dynBase": MFn.kDynBase, "dynController": MFn.kDynamicsController,
    "dynGlobals": MFn.kDynGlobals, "dynHolder": MFn.kDummy, "dynamicConstraint": MFn.kDynamicConstraint,
    "editMetadata": MFn.kEditMetadata, "editsManager": MFn.kEditsManager, "entity": MFn.kContainerBase,
    "envBall": MFn.kEnvBall, "envChrome": MFn.kEnvChrome, "envCube": MFn.kEnvCube, "envFacade": MFn.kEnvFacade,
    "envFog": MFn.kEnvFogMaterial, "envSky": MFn.kEnvSky, "envSphere": MFn.kEnvSphere,
    "environmentFog": MFn.kEnvFogShape, "equal": MFn.kEqual, "equalDL": MFn.kEqual,
    "explodeNurbsShell": MFn.kExplodeNurbsShell, "expression": MFn.kExpression, "extendCurve": MFn.kExtendCurve,
    "extendSurface": MFn.kExtendSurface, "extrude": MFn.kExtrude, "facade": MFn.kFacade,
    "falloffEval": MFn.kFalloffEval, "ffBlendSrf": MFn.kFFblendSrf, "ffBlendSrfObsolete": MFn.kFFblendSrf,
    "ffFilletSrf": MFn.kFFfilletSrf, "ffd": MFn.kFFD, "field": MFn.kField, "file": MFn.kFileTexture,
    "filletCurve": MFn.kFilletCurve, "fitBspline": MFn.kFitBspline, "flexorShape": MFn.kFlexor, "floor": MFn.kFloor,
    "floorDL": MFn.kFloor, "flow": MFn.kFlow, "fluidEmitter": MFn.kFluidEmitter, "fluidShape": MFn.kFluid,
    "fluidTexture2D": MFn.kFluidTexture2D, "fluidTexture3D": MFn.kFluidTexture3D, "follicle": MFn.kFollicle,
    "fosterParent": MFn.kFosterParent, "fourByFourMatrix": MFn.kFourByFourMatrix, "fractal": MFn.kFractal,
    "frameCache": MFn.kTime, "gammaCorrect": MFn.kGammaCorrect, "geoConnectable": MFn.kGeoConnectable,
    "geoConnector": MFn.kGeoConnector, "geomBind": MFn.kGeomBind, "geometryConstraint": MFn.kGeometryConstraint,
    "geometryFilter": MFn.kGeometryFilt, "geometryShape": MFn.kShape, "geometryVarGroup": MFn.kAnyGeometryVarGroup,
    "globalCacheControl": MFn.kGlobalCacheControls, "globalStitch": MFn.kGlobalStitch, "granite": MFn.kGranite,
    "gravityField": MFn.kGravity, "greasePencilSequence": MFn.kGreasePencilSequence, "greasePlane": MFn.kGreasePlane,
    "greasePlaneRenderShape": MFn.kGreasePlaneRenderShape, "greaterThan": MFn.kGreaterThan,
    "greaterThanDL": MFn.kGreaterThan, "grid": MFn.kGrid, "groundPlane": MFn.kGroundPlane, "groupId": MFn.kGroupId,
    "groupParts": MFn.kGroupParts, "guide": MFn.kGuide, "hairConstraint": MFn.kHairConstraint,
    "hairSystem": MFn.kHairSystem, "hairTubeShader": MFn.kHairTubeShader, "hardenPoint": MFn.kHardenPointCurve,
    "hardwareRenderGlobals": MFn.kHardwareRenderGlobals, "hardwareRenderingGlobals": MFn.kHardwareRenderingGlobals,
    "heightField": MFn.kHeightField, "hierarchyTestNode1": MFn.kDependencyNode,
    "hierarchyTestNode2": MFn.kDependencyNode, "hierarchyTestNode3": MFn.kDependencyNode,
    "hierarchyTestNode4": MFn.kDependencyNode, "hikEffector": MFn.kHikEffector, "hikFKJoint": MFn.kHikFKJoint,
    "hikFloorContactMarker": MFn.kHikFloorContactMarker, "hikGroundPlane": MFn.kHikGroundPlane,
    "hikHandle": MFn.kHikHandle, "hikIKEffector": MFn.kHikIKEffector, "hikSolver": MFn.kHikSolver,
    "historySwitch": MFn.kHistorySwitch, "holdMatrix": MFn.kMatrixHold, "hsvToRgb": MFn.kHsvToRgb,
    "hwReflectionMap": MFn.kHardwareReflectionMap, "hwRenderGlobals": MFn.kHardwareRenderGlobals,
    "hwShader": MFn.kHwShaderNode, "hyperGraphInfo": MFn.kHyperGraphInfo, "hyperLayout": MFn.kHyperLayout,
    "hyperView": MFn.kHyperView, "ikEffector": MFn.kIkEffector, "ikHandle": MFn.kIkHandle,
    "ikMCsolver": MFn.kMCsolver, "ikPASolver": MFn.kPASolver, "ikRPsolver": MFn.kRPsolver,
    "ikSCsolver": MFn.kSCsolver, "ikSolver": MFn.kIkSolver, "ikSplineSolver": MFn.kSplineSolver,
    "ikSystem": MFn.kIkSystem, "imagePlane": MFn.kImagePlane, "imageSource": MFn.kImageSource,
    "implicitBox": MFn.kBox, "implicitCone": MFn.kImplicitCone, "implicitSphere": MFn.kImplicitSphere,
    "insertKnotCurve": MFn.kInsertKnotCrv, "insertKnotSurface": MFn.kInsertKnotSrf, "instancer": MFn.kInstancer,
    "intersectSurface": MFn.kIntersectSurface, "inverseLerp": MFn.kInverseLinearInterpolation,
    "inverseLerpDL": MFn.kInverseLinearInterpolation, "jiggle": MFn.kJiggleDeformer, "joint": MFn.kJoint,
    "jointCluster": MFn.kJointCluster, "jointFfd": MFn.kFfdDualBase, "jointLattice": MFn.kBendLattice,
    "keyingGroup": MFn.kKeyingGroup, "lambert": MFn.kLambert, "lattice": MFn.kLattice,
    "layeredShader": MFn.kLayeredShader, "layeredTexture": MFn.kLayeredTexture,
    "leastSquaresModifier": MFn.kLeastSquares, "leather": MFn.kLeather, "length": MFn.kLength,
    "lengthDL": MFn.kLength, "lerp": MFn.kLinearInterpolation, "lerpDL": MFn.kLinearInterpolation,
    "lessThan": MFn.kLessThan, "lessThanDL": MFn.kLessThan, "light": MFn.kLight, "lightFog": MFn.kLightFogMaterial,
    "lightInfo": MFn.kLightInfo, "lightLinker": MFn.kLightLink, "lightList": MFn.kLightList,
    "lineModifier": MFn.kLineModifier, "locator": MFn.kLocator, "lodGroup": MFn.kLodGroup,
    "lodThresholds": MFn.kLodThresholds, "loft": MFn.kSkin, "log": MFn.kLog, "logDL": MFn.kLog, "lookAt": MFn.kLookAt,
    "luminance": MFn.kLuminance, "makeCircularArc": MFn.kCreate, "makeGroup": MFn.kMakeGroup,
    "makeIllustratorCurves": MFn.kIllustratorCurve, "makeNurbCircle": MFn.kCircle, "makeNurbCone": MFn.kCone,
    "makeNurbCube": MFn.kNurbsCube, "makeNurbCylinder": MFn.kCylinder, "makeNurbPlane": MFn.kNurbsPlane,
    "makeNurbSphere": MFn.kSphere, "makeNurbTorus": MFn.kTorus, "makeNurbsSquare": MFn.kNurbsSquare,
    "makeTextCurves": MFn.kTextCurves, "makeThreePointCircularArc": MFn.kNurbsCircular3PtArc,
    "makeTwoPointCircularArc": MFn.kNurbsCircular2PtArc, "mandelbrot": MFn.kMandelbrot,
    "mandelbrot3D": MFn.kMandelbrot3D, "marble": MFn.kMarble, "materialFacade": MFn.kMaterialFacade,
    "materialInfo": MFn.kMaterialInfo, "materialTemplate": MFn.kMaterialTemplate, "max": MFn.kMax, "maxDL": MFn.kMax,
    "membrane": MFn.kMembrane, "mesh": MFn.kMesh, "meshVarGroup": MFn.kMeshVarGroup, "min": MFn.kMin,
    "minDL": MFn.kMin, "modulo": MFn.kModulo, "moduloDL": MFn.kModulo, "morph": MFn.kMorph,
    "motionPath": MFn.kMotionPath, "motionTrail": MFn.kSnapshotPath, "motionTrailShape": MFn.kSnapshot,
    "mountain": MFn.kMountain, "moveVertexManip": MFn.kMoveVertexManip, "movie": MFn.kFileTexture,
    "mpBirailSrf": MFn.kMPbirailSrf, "multDoubleLinear": MFn.kMultDoubleLinear, "multMatrix": MFn.kMatrixMult,
    "multilisterLight": MFn.kMultilisterLight, "multiply": MFn.kMultiply, "multiplyDL": MFn.kMultiply,
    "multiplyDivide": MFn.kMultiplyDivide, "multiplyPointByMatrix": MFn.kMultiplyPointByMatrix,
    "multiplyPointByMatrixDL": MFn.kMultiplyPointByMatrix, "multiplyVectorByMatrix": MFn.kMultiplyVectorByMatrix,
    "multiplyVectorByMatrixDL": MFn.kMultiplyVectorByMatrix, "mute": MFn.kMute, "nBase": MFn.kNBase,
    "nCloth": MFn.kNCloth, "nComponent": MFn.kNComponent, "nParticle": MFn.kNParticle, "nRigid": MFn.kNRigid,
    "nearestPointOnCurve": MFn.kNearestPointOnCurve, "negate": MFn.kNegate, "negateDL": MFn.kNegate,
    "network": MFn.kAffect, "newtonField": MFn.kNewton, "node": MFn.kDependencyNode,
    "nodeGraphEditorBookmarkInfo": MFn.kNodeGraphEditorBookmarkInfo,
    "nodeGraphEditorBookmarks": MFn.kNodeGraphEditorBookmarks, "nodeGraphEditorInfo": MFn.kNodeGraphEditorInfo,
    "noise": MFn.kNoise, "nonAmbientLightShapeNode": MFn.kNonAmbientLight,
    "nonExtendedLightShapeNode": MFn.kNonExtendedLight, "nonLinear": MFn.kNonLinear,
    "normalConstraint": MFn.kNormalConstraint, "normalize": MFn.kNormalize, "normalizeDL": MFn.kNormalize,
    "not": MFn.kNot, "nucleus": MFn.kNucleus, "nurbsCurve": MFn.kNurbsCurve,
    "nurbsCurveToBezier": MFn.kNurbsCurveToBezier, "nurbsDimShape": MFn.kDimension, "nurbsSurface": MFn.kNurbsSurface,
    "nurbsTessellate": MFn.kNurbsTesselate, "nurbsToSubdiv": MFn.kNurbsToSubdiv,
    "nurbsToSubdivProc": MFn.kNurbsToSubdiv, "objectAttrFilter": MFn.kObjectAttrFilter,
    "objectBinFilter": MFn.kObjectBinFilter, "objectFilter": MFn.kObjectFilter,
    "objectMultiFilter": MFn.kObjectMultiFilter, "objectNameFilter": MFn.kObjectNameFilter,
    "objectRenderFilter": MFn.kObjectRenderFilter, "objectScriptFilter": MFn.kObjectScriptFilter,
    "objectSet": MFn.kSet, "objectTypeFilter": MFn.kObjectTypeFilter, "ocean": MFn.kOcean,
    "oceanShader": MFn.kOceanShader, "offsetCos": MFn.kOffsetCos, "offsetCurve": MFn.kOffsetCurve,
    "offsetSurface": MFn.kOffsetSurface, "oldBlindDataBase": MFn.kDependencyNode,
    "oldGeometryConstraint": MFn.kOldGeometryConstraint, "oldNormalConstraint": MFn.kNormalConstraint,
    "oldTangentConstraint": MFn.kTangentConstraint, "opticalFX": MFn.kOpticalFX, "or": MFn.kOr,
    "orientConstraint": MFn.kOrientConstraint, "orientationMarker": MFn.kOrientationMarker,
    "orthoGrid": MFn.kOrthoGrid, "paintableShadingDependNode": MFn.kDependencyNode, "pairBlend": MFn.kPairBlend,
    "paramDimension": MFn.kParamDimension, "parentConstraint": MFn.kParentConstraint,
    "parentMatrix": MFn.kParentMatrix, "parentTessellate": MFn.kCreate, "particle": MFn.kParticle,
    "particleAgeMapper": MFn.kParticleAgeMapper, "particleCloud": MFn.kParticleCloud,
    "particleColorMapper": MFn.kParticleColorMapper, "particleIncandMapper": MFn.kParticleIncandecenceMapper,
    "particleSamplerInfo": MFn.kParticleSamplerInfo, "particleTranspMapper": MFn.kParticleTransparencyMapper,
    "partition": MFn.kPartition, "passContributionMap": MFn.kPassContributionMap, "passMatrix": MFn.kMatrixPass,
    "pfxGeometry": MFn.kPfxGeometry, "pfxHair": MFn.kPfxHair, "pfxToon": MFn.kPfxToon, "phong": MFn.kPhong,
    "phongE": MFn.kPhongExplorer, "pi": MFn.kPIConstant, "pickMatrix": MFn.kPickMatrix,
    "place2dTexture": MFn.kPlace2dTexture, "place3dTexture": MFn.kPlace3dTexture,
    "planarTrimSurface": MFn.kPlanarTrimSrf, "plane": MFn.kPlane, "plusMinusAverage": MFn.kPlusMinusAverage,
    "pointConstraint": MFn.kPointConstraint, "pointEmitter": MFn.kEmitter, "pointLight": MFn.kPointLight,
    "pointMatrixMult": MFn.kPointMatrixMult, "pointMatrixMultDL": MFn.kPointMatrixMult,
    "pointOnCurveInfo": MFn.kPointOnCurveInfo, "pointOnPolyConstraint": MFn.kPointOnPolyConstraint,
    "pointOnSurfaceInfo": MFn.kPointOnSurfaceInfo, "poleVectorConstraint": MFn.kPoleVectorConstraint,
    "polyAppend": MFn.kPolyAppend, "polyAppendVertex": MFn.kPolyAppendVertex, "polyAutoProj": MFn.kPolyAutoProj,
    "polyAverageVertex": MFn.kPolyAverageVertex, "polyAxis": MFn.kPolyAxis, "polyBase": MFn.kDependencyNode,
    "polyBevel": MFn.kPolyBevel, "polyBevel2": MFn.kPolyBevel2, "polyBevel3": MFn.kPolyBevel3,
    "polyBevelCutback": MFn.kPolyBevelCutback, "polyBlindData": MFn.kPolyBlindData, "polyBoolOp": MFn.kPolyBoolOp,
    "polyBoolean": MFn.kMidModifier, "polyBridgeEdge": MFn.kPolyBridgeEdge, "polyCBoolOp": MFn.kPolyCBoolOp,
    "polyChipOff": MFn.kPolyChipOff, "polyCircularize": MFn.kPolyCircularize, "polyClean": MFn.kPolyClean,
    "polyCloseBorder": MFn.kPolyCloseBorder, "polyCollapseEdge": MFn.kPolyCollapseEdge,
    "polyCollapseF": MFn.kPolyCollapseF, "polyColorDel": MFn.kPolyColorDel, "polyColorMod": MFn.kPolyColorMod,
    "polyColorPerVertex": MFn.kPolyColorPerVertex, "polyCone": MFn.kPolyCone,
    "polyConnectComponents": MFn.kPolyConnectComponents, "polyContourProj": MFn.kPolyContourProj,
    "polyCopyUV": MFn.kMidModifier, "polyCrease": MFn.kPolyCreaseEdge, "polyCreaseEdge": MFn.kPolyCreaseEdge,
    "polyCreateFace": MFn.kPolyCreateFacet, "polyCreator": MFn.kPolyCreator, "polyCube": MFn.kPolyCube,
    "polyCut": MFn.kPolyCut, "polyCylProj": MFn.kPolyCylProj, "polyCylinder": MFn.kPolyCylinder,
    "polyDelEdge": MFn.kPolyDelEdge, "polyDelFacet": MFn.kPolyDelFacet, "polyDelVertex": MFn.kPolyDelVertex,
    "polyDigits": MFn.kPolyCreator, "polyDual": MFn.kMidModifier, "polyDuplicateEdge": MFn.kPolyDuplicateEdge,
    "polyEdgeToCurve": MFn.kPolyEdgeToCurve, "polyEditEdgeFlow": MFn.kPolyEditEdgeFlow,
    "polyExtrudeEdge": MFn.kPolyExtrudeEdge, "polyExtrudeFace": MFn.kPolyExtrudeFacet,
    "polyExtrudeVertex": MFn.kPolyExtrudeVertex, "polyFlipEdge": MFn.kPolyFlipEdge, "polyFlipUV": MFn.kPolyFlipUV,
    "polyHelix": MFn.kPolyHelix, "polyHoleFace": MFn.kPolyHoleFace, "polyLayoutUV": MFn.kPolyLayoutUV,
    "polyMapCut": MFn.kPolyMapCut, "polyMapDel": MFn.kPolyMapDel, "polyMapSew": MFn.kPolyMapSew,
    "polyMapSewMove": MFn.kPolyMapSewMove, "polyMergeEdge": MFn.kPolyMergeEdge, "polyMergeFace": MFn.kPolyMergeFacet,
    "polyMergeUV": MFn.kPolyMergeUV, "polyMergeVert": MFn.kPolyMergeVert, "polyMirror": MFn.kPolyMirror,
    "polyModifier": MFn.kDependencyNode, "polyModifierUV": MFn.kDependencyNode,
    "polyModifierWorld": MFn.kDependencyNode, "polyMoveEdge": MFn.kPolyMoveEdge, "polyMoveFace": MFn.kPolyMoveFacet,
    "polyMoveFacetUV": MFn.kPolyMoveFacetUV, "polyMoveUV": MFn.kPolyMoveUV, "polyMoveVertex": MFn.kPolyMoveVertex,
    "polyNormal": MFn.kPolyNormal, "polyNormalPerVertex": MFn.kPolyNormalPerVertex,
    "polyNormalizeUV": MFn.kPolyNormalizeUV, "polyOptUvs": MFn.kMidModifier, "polyPassThru": MFn.kPolyPassThru,
    "polyPinUV": MFn.kPolyPinUV, "polyPipe": MFn.kPolyPipe, "polyPlanarProj": MFn.kPolyPlanProj,
    "polyPlane": MFn.kPolyMesh, "polyPlatonicSolid": MFn.kPolyPlatonicSolid, "polyPoke": MFn.kPolyPoke,
    "polyPrimitive": MFn.kPolyPrimitive, "polyPrimitiveMisc": MFn.kPolyPrimitiveMisc, "polyPrism": MFn.kPolyPrism,
    "polyProj": MFn.kPolyProj, "polyProjectCurve": MFn.kPolyProjectCurve, "polyPyramid": MFn.kPolyPyramid,
    "polyQuad": MFn.kPolyQuad, "polyReduce": MFn.kPolyReduce, "polyRemesh": MFn.kPolyRemesh,
    "polyRetopo": MFn.kReForm, "polySeparate": MFn.kPolySeparate, "polySewEdge": MFn.kPolySewEdge,
    "polySmartExtrude": MFn.kPolySmartExtrude, "polySmooth": MFn.kPolySmooth, "polySmoothFace": MFn.kPolySmoothFacet,
    "polySmoothProxy": MFn.kPolySmoothProxy, "polySoftEdge": MFn.kPolySoftEdge, "polySphProj": MFn.kPolySphProj,
    "polySphere": MFn.kPolySphere, "polySpinEdge": MFn.kPolySpinEdge, "polySplit": MFn.kPolySplit,
    "polySplitEdge": MFn.kPolySplitEdge, "polySplitRing": MFn.kPolySplitRing, "polySplitVert": MFn.kPolySplitVert,
    "polyStraightenUVBorder": MFn.kPolyStraightenUVBorder, "polySubdEdge": MFn.kPolySubdEdge,
    "polySubdFace": MFn.kPolySubdFacet, "polySubdFaceAny": MFn.kMidModifier, "polyToSubdiv": MFn.kPolyToSubdiv,
    "polyTorus": MFn.kPolyTorus, "polyTransfer": MFn.kPolyTransfer, "polyTriPatch": MFn.kMidModifier,
    "polyTriangulate": MFn.kPolyTriangulate, "polyTweak": MFn.kPolyTweak, "polyTweakUV": MFn.kPolyTweakUV,
    "polyUVRectangle": MFn.kPolyUVRectangle, "polyUnite": MFn.kPolyUnite, "polyUnsmooth": MFn.kPolyUnsmooth,
    "polyWedgeFace": MFn.kPolyWedgeFace, "poseInterpolatorManager": MFn.kPoseInterpolatorManager,
    "positionMarker": MFn.kPositionMarker, "postProcessList": MFn.kPostProcessList, "power": MFn.kPower,
    "powerDL": MFn.kPower, "precompExport": MFn.kPrecompExport, "primitive": MFn.kPrimitive,
    "primitiveFalloff": MFn.kPrimitiveFalloff, "projectCurve": MFn.kProjectCurve,
    "projectTangent": MFn.kProjectTangent, "projection": MFn.kProjection, "proximityFalloff": MFn.kProximityFalloff,
    "proximityPin": MFn.kProximityPin, "proximityWrap": MFn.kProximityWrap, "proxyManager": MFn.kProxyManager,
    "psdFileTex": MFn.kPsdFileTexture, "quadShadingSwitch": MFn.kQuadShadingSwitch, "radialField": MFn.kRadial,
    "ramp": MFn.kRamp, "rampShader": MFn.kRampShader, "rbfSrf": MFn.kRBFsurface, "rebuildCurve": MFn.kRebuildCurve,
    "rebuildSurface": MFn.kRebuildSurface, "record": MFn.kRecord, "reference": MFn.kReference,
    "reflect": MFn.kReflect, "remapColor": MFn.kRemapColor, "remapHsv": MFn.kRemapHsv, "remapValue": MFn.kRemapValue,
    "renderBox": MFn.kRenderBox, "renderCone": MFn.kRenderCone, "renderGlobals": MFn.kRenderGlobals,
    "renderGlobalsList": MFn.kRenderGlobalsList, "renderLayer": MFn.kRenderLayer,
    "renderLayerManager": MFn.kRenderLayerManager, "renderLight": MFn.kLight, "renderPass": MFn.kRenderPass,
    "renderPassSet": MFn.kRenderPassSet, "renderQuality": MFn.kRenderQuality, "renderRect": MFn.kRenderRect,
    "renderSphere": MFn.kRenderSphere, "renderTarget": MFn.kRenderTarget,
    "renderedImageSource": MFn.kRenderedImageSource, "reorderUVSet": MFn.kReorderUVSet, "resolution": MFn.kResolution,
    "resultCurve": MFn.kResultCurve, "resultCurveTimeToAngular": MFn.kResultCurveTimeToAngular,
    "resultCurveTimeToLinear": MFn.kResultCurveTimeToDistance, "resultCurveTimeToTime": MFn.kResultCurveTimeToTime,
    "resultCurveTimeToUnitless": MFn.kResultCurveTimeToUnitless, "reverse": MFn.kReverse,
    "reverseCurve": MFn.kReverseCurve, "reverseSurface": MFn.kReverseSurface, "revolve": MFn.kRevolve,
    "revolvedPrimitive": MFn.kRevolvedPrimitive, "rgbToHsv": MFn.kRgbToHsv, "rigidBody": MFn.kRigid,
    "rigidConstraint": MFn.kRigidConstraint, "rigidSolver": MFn.kRigidSolver, "rock": MFn.kRock,
    "rotateVector": MFn.kRotateVector, "rotateVectorDL": MFn.kRotateVector,
    "rotationFromMatrix": MFn.kRotationFromMatrix, "round": MFn.kRound,
    "roundConstantRadius": MFn.kRoundConstantRadius, "roundDL": MFn.kRound, "rowFromMatrix": MFn.kRowFromMatrix,
    "rowFromMatrixDL": MFn.kRowFromMatrix, "sampler": MFn.kTime, "samplerInfo": MFn.kSamplerInfo,
    "scaleConstraint": MFn.kScaleConstraint, "scaleFromMatrix": MFn.kScaleFromMatrix,
    "scaleFromMatrixDL": MFn.kScaleFromMatrix, "script": MFn.kScript, "sculpt": MFn.kSculpt,
    "selectionListOperator": MFn.kSelectionListOperator, "sequenceManager": MFn.kSequenceManager,
    "sequencer": MFn.kSequencer, "setRange": MFn.kSetRange, "shaderGlow": MFn.kShaderGlow,
    "shadingDependNode": MFn.kDependencyNode, "shadingEngine": MFn.kShadingEngine, "shadingMap": MFn.kShadingMap,
    "shape": MFn.kShape, "shapeEditorManager": MFn.kShapeEditorManager, "shellTessellate": MFn.kNurbsTesselate,
    "shot": MFn.kShot, "shrinkWrap": MFn.kShrinkWrapFilter, "simpleTestNode": MFn.kDependencyNode,
    "simpleVolumeShader": MFn.kSimpleVolumeShader, "sin": MFn.kSin, "sinDL": MFn.kSin,
    "singleShadingSwitch": MFn.kSingleShadingSwitch, "sketchPlane": MFn.kSketchPlane, "skinBinding": MFn.kSkinBinding,
    "skinCluster": MFn.kSkinClusterFilter, "smear": MFn.kSmear, "smoothCurve": MFn.kSmoothCurve,
    "smoothStep": MFn.kSmoothStep, "smoothStepDL": MFn.kSmoothStep, "smoothTangentSrf": MFn.kSmoothTangentSrf,
    "snapshot": MFn.kSnapshotPath, "snapshotShape": MFn.kSnapshot, "snow": MFn.kSnow, "softMod": MFn.kSoftModFilter,
    "softModHandle": MFn.kSoftMod, "solidFractal": MFn.kSolidFractal, "solidify": MFn.kSolidify,
    "spBirailSrf": MFn.kSPbirailSrf, "spotLight": MFn.kSpotLight, "spring": MFn.kSpring, "squareSrf": MFn.kSquareSrf,
    "standardSurface": MFn.kStandardSurface, "stencil": MFn.kStencil, "stereoRigCamera": MFn.kStereoCameraMaster,
    "stitchAsNurbsShell": MFn.kStitchAsNurbsShell, "stitchSrf": MFn.kStitchSrf, "stroke": MFn.kStroke,
    "strokeGlobals": MFn.kStrokeGlobals, "stucco": MFn.kStucco, "studioClearCoat": MFn.kStudioClearCoat,
    "styleCurve": MFn.kStyleCurve, "subCurve": MFn.kSubCurve, "subSurface": MFn.kSubSurface,
    "subdAddTopology": MFn.kSubdAddTopology, "subdAutoProj": MFn.kSubdAutoProj, "subdBase": MFn.kDependencyNode,
    "subdBlindData": MFn.kSubdBlindData, "subdCleanTopology": MFn.kSubdCleanTopology,
    "subdHierBlind": MFn.kSubdHierBlind, "subdLayoutUV": MFn.kSubdLayoutUV, "subdMapCut": MFn.kSubdMapCut,
    "subdMapSewMove": MFn.kSubdMapSewMove, "subdModifier": MFn.kSubdModifier, "subdModifierUV": MFn.kSubdModifier,
    "subdModifierWorld": MFn.kSubdModifier, "subdPlanarProj": MFn.kSubdPlanProj, "subdTweak": MFn.kSubdTweak,
    "subdTweakUV": MFn.kSubdTweakUV, "subdiv": MFn.kSubdiv, "subdivCollapse": MFn.kSubdivCollapse,
    "subdivComponentId": MFn.kSubdivCompId, "subdivReverseFaces": MFn.kSubdivReverseFaces,
    "subdivSurfaceVarGroup": MFn.kSurfaceVarGroup, "subdivToNurbs": MFn.kSubdivToNurbs,
    "subdivToPoly": MFn.kSubdivToPoly, "subsetFalloff": MFn.kSubsetFalloff, "subtract": MFn.kSubtract,
    "subtractDL": MFn.kSubtract, "sum": MFn.kSum, "sumDL": MFn.kSum, "sunDirection": MFn.kDependencyNode,
    "surfaceInfo": MFn.kSurfaceInfo, "surfaceLuminance": MFn.kSurfaceLuminance, "surfaceShader": MFn.kSurfaceShader,
    "surfaceShape": MFn.kSurface, "surfaceVarGroup": MFn.kSurfaceVarGroup,
    "symmetryConstraint": MFn.kSymmetryConstraint, "tan": MFn.kTan, "tanDL": MFn.kTan,
    "tangentConstraint": MFn.kTangentConstraint, "tension": MFn.kTension, "texLattice": MFn.kTexLattice,
    "texture2d": MFn.kTexture2d, "texture3d": MFn.kTexture3d, "textureBakeSet": MFn.kTextureBakeSet,
    "textureDeformer": MFn.kTextureDeformer, "textureDeformerHandle": MFn.kTextureDeformerHandle,
    "textureEnv": MFn.kTextureEnv, "textureToGeom": MFn.kDependencyNode, "threadedDevice": MFn.kThreadedDevice,
    "time": MFn.kTime, "timeEditor": MFn.kTimeEditor, "timeEditorAnimSource": MFn.kTimeEditorAnimSource,
    "timeEditorClip": MFn.kTimeEditorClip, "timeEditorClipBase": MFn.kTimeEditorClipBase,
    "timeEditorClipEvaluator": MFn.kTimeEditorClipEvaluator, "timeEditorInterpolator": MFn.kTimeEditorInterpolator,
    "timeEditorTracks": MFn.kTimeEditorTracks, "timeFunction": MFn.kTimeFunction,
    "timeSliderBookmark": MFn.kDependencyNode, "timeToUnitConversion": MFn.kTimeToUnitConversion,
    "timeWarp": MFn.kTimeWarp, "toonLineAttributes": MFn.kToonLineAttributes,
    "trackInfoManager": MFn.kTrackInfoManager, "transferAttributes": MFn.kTransferAttributes,
    "transferFalloff": MFn.kTransferFalloff, "transform": MFn.kTransform, "transformGeometry": MFn.kTransformGeometry,
    "translationFromMatrix": MFn.kTranslationFromMatrix, "translationFromMatrixDL": MFn.kTranslationFromMatrix,
    "trim": MFn.kTrim, "trimWithBoundaries": MFn.kTrimWithBoundaries, "tripleShadingSwitch": MFn.kTripleShadingSwitch,
    "truncate": MFn.kTruncate, "truncateDL": MFn.kTruncate, "turbulenceField": MFn.kTurbulence, "tweak": MFn.kTweak,
    "ufeProxyCameraShape": MFn.kCamera, "ufeProxyTransform": MFn.kUfeProxyTransform,
    "uniformFalloff": MFn.kUniformFalloff, "uniformField": MFn.kUniform, "unitConversion": MFn.kUnitConversion,
    "unitToTimeConversion": MFn.kUnitToTimeConversion, "unknown": MFn.kUnknown, "unknownDag": MFn.kUnknownDag,
    "unknownTransform": MFn.kUnknownTransform, "untrim": MFn.kUntrim, "useBackground": MFn.kUseBackground,
    "uvChooser": MFn.kUvChooser, "uvPin": MFn.kUVPin, "vectorProduct": MFn.kVectorProduct,
    "vectorRenderGlobals": MFn.kDependencyNode, "vertexBakeSet": MFn.kVertexBakeSet,
    "viewColorManager": MFn.kViewColorManager, "volumeAxisField": MFn.kVolumeAxis, "volumeFog": MFn.kVolumeFog,
    "volumeLight": MFn.kVolumeLight, "volumeNoise": MFn.kVolumeNoise, "volumeShader": MFn.kVolumeShader,
    "vortexField": MFn.kVortex, "water": MFn.kWater, "weightGeometryFilter": MFn.kWeightGeometryFilt,
    "wire": MFn.kWire, "wood": MFn.kWood, "wrap": MFn.kWrapFilter, "wtAddMatrix": MFn.kMatrixWtAdd,
    "xformManip": MFn.kXformManip,
}


# Node types the API knows by name, with what else each answers to.
_FN_OF_TYPE = dict(_REF_FN_OF_TYPE)
_FN_OF_TYPE.update({
    "dagNode": MFn.kDagNode,
    "shape": MFn.kShape,
    "transform": MFn.kTransform,
    "joint": MFn.kJoint,
    "mesh": MFn.kMesh,
    "nurbsCurve": MFn.kNurbsCurve,
    "locator": MFn.kLocator,
    "camera": MFn.kCamera,
    "ikHandle": MFn.kIkHandle,
    "ikEffector": MFn.kIkEffector,
    "nurbsSurface": MFn.kNurbsSurface,
    "skinCluster": MFn.kSkinClusterFilter,
    "blendShape": MFn.kBlendShape,
    "cluster": MFn.kCluster,
    "nonLinear": MFn.kNonLinear,
    "objectSet": MFn.kSet,
    "displayLayer": MFn.kDisplayLayer,
    "time": MFn.kTime,
    "polyCube": MFn.kPolyCube,
    "polySphere": MFn.kPolySphere,
    "polyCylinder": MFn.kPolyCylinder,
    "polyCone": MFn.kPolyCone,
    "polyPlane": MFn.kPolyMesh,
    "polyTorus": MFn.kPolyTorus,
    "polyTweak": MFn.kPolyTweak,
    "animCurveTL": MFn.kAnimCurveTimeToDistance,
    "animCurveTA": MFn.kAnimCurveTimeToAngular,
    "animCurveTT": MFn.kAnimCurveTimeToTime,
    "animCurveTU": MFn.kAnimCurveTimeToUnitless,
    "animCurveUL": MFn.kAnimCurveUnitlessToDistance,
    "animCurveUA": MFn.kAnimCurveUnitlessToAngular,
    "animCurveUT": MFn.kAnimCurveUnitlessToTime,
    "animCurveUU": MFn.kAnimCurveUnitlessToUnitless,
    "aimConstraint": MFn.kAimConstraint,
    "orientConstraint": MFn.kOrientConstraint,
    "parentConstraint": MFn.kParentConstraint,
    "pointConstraint": MFn.kPointConstraint,
    "scaleConstraint": MFn.kScaleConstraint,
    "multiplyDivide": MFn.kMultiplyDivide,
})
_FN_BASES = {
    MFn.kMesh: [MFn.kSurface, MFn.kGeometric, MFn.kShape, MFn.kDagNode],
    MFn.kNurbsCurve: [MFn.kCurve, MFn.kGeometric, MFn.kShape, MFn.kDagNode],
    MFn.kLocator: [MFn.kShape, MFn.kDagNode],
    MFn.kCamera: [MFn.kShape, MFn.kDagNode],
    MFn.kShape: [MFn.kDagNode],
    MFn.kJoint: [MFn.kTransform, MFn.kDagNode],
    MFn.kIkHandle: [MFn.kTransform, MFn.kDagNode],
    MFn.kIkEffector: [MFn.kTransform, MFn.kDagNode],
    MFn.kNurbsSurface: [MFn.kSurface, MFn.kGeometric, MFn.kShape, MFn.kDagNode],
    MFn.kTransform: [MFn.kDagNode],
        MFn.kSkinClusterFilter: [MFn.kGeometryFilt],
    MFn.kBlendShape: [MFn.kGeometryFilt],
    MFn.kCluster: [MFn.kGeometryFilt],
    MFn.kAnimCurveTimeToDistance: [MFn.kAnimCurve],
    MFn.kAnimCurveTimeToAngular: [MFn.kAnimCurve],
    MFn.kAnimCurveTimeToTime: [MFn.kAnimCurve],
    MFn.kAnimCurveTimeToUnitless: [MFn.kAnimCurve],
    MFn.kAnimCurveUnitlessToDistance: [MFn.kAnimCurve],
    MFn.kAnimCurveUnitlessToAngular: [MFn.kAnimCurve],
    MFn.kAnimCurveUnitlessToTime: [MFn.kAnimCurve],
    MFn.kAnimCurveUnitlessToUnitless: [MFn.kAnimCurve],
    MFn.kAimConstraint: [MFn.kConstraint],
    MFn.kOrientConstraint: [MFn.kConstraint],
    MFn.kParentConstraint: [MFn.kConstraint],
    MFn.kPointConstraint: [MFn.kConstraint],
    MFn.kScaleConstraint: [MFn.kConstraint],
    MFn.kPolyCube: [MFn.kPolyPrimitive],
    MFn.kPolySphere: [MFn.kPolyPrimitive],
    MFn.kPolyCylinder: [MFn.kPolyPrimitive],
    MFn.kPolyCone: [MFn.kPolyPrimitive],
    MFn.kPolyMesh: [MFn.kPolyPrimitive],
    MFn.kPolyTorus: [MFn.kPolyPrimitive],
    MFn.kMeshVertComponent: [MFn.kMeshComponent, MFn.kSingleIndexedComponent, MFn.kComponent],
    MFn.kMeshEdgeComponent: [MFn.kMeshComponent, MFn.kSingleIndexedComponent, MFn.kComponent],
    MFn.kMeshPolygonComponent: [MFn.kMeshComponent, MFn.kSingleIndexedComponent, MFn.kComponent],
    MFn.kMeshMapComponent: [MFn.kMeshComponent, MFn.kSingleIndexedComponent, MFn.kComponent],
    MFn.kMeshVtxFaceComponent: [MFn.kMeshComponent, MFn.kDoubleIndexedComponent, MFn.kComponent],
    MFn.kCurveCVComponent: [MFn.kSingleIndexedComponent, MFn.kComponent],
    MFn.kCurveEPComponent: [MFn.kSingleIndexedComponent, MFn.kComponent],
}
_FN_NAMES = {v: k for k, v in reversed(list(MFn.__dict__.items())) if k.startswith("k") and isinstance(v, int)}

# Components by the name they go by in strings.
_COMP_TYPES = {
    "vtx": MFn.kMeshVertComponent,
    "e": MFn.kMeshEdgeComponent,
    "f": MFn.kMeshPolygonComponent,
    "map": MFn.kMeshMapComponent,
    "vtxFace": MFn.kMeshVtxFaceComponent,
    "cv": MFn.kCurveCVComponent,
    "ep": MFn.kCurveEPComponent,
}
_COMP_NAMES = {v: k for k, v in _COMP_TYPES.items()}
_COMP_RE = _re.compile(r"^(.+)\.(vtxFace|vtx|map|cv|ep|e|f)\[([^\]]*)\](?:\[([^\]]*)\])?$")

# Rotation attributes, by every spelling.
_ANGLE_ATTRS = {
    "rotate", "rotateX", "rotateY", "rotateZ", "r", "rx", "ry", "rz",
    "jointOrient", "jointOrientX", "jointOrientY", "jointOrientZ", "jo", "jox", "joy", "joz",
    "rotateAxis", "rotateAxisX", "rotateAxisY", "rotateAxisZ", "ra", "rax", "ray", "raz",
}
# One-element arrays in the reference that are single plugs here.
_ONE_ELEMENT = {"worldMatrix", "wm", "parentMatrix", "pm", "worldInverseMatrix", "wim", "parentInverseMatrix", "pim", "worldSpace", "ws"}


def _fn_chain(info):
    fns = []
    for t in info["chain"]:
        f = _FN_OF_TYPE.get(t)
        if f is not None and f not in fns:
            fns.append(f)
            for b in _FN_BASES.get(f, []):
                if b not in fns:
                    fns.append(b)
    if info.get("owner"):
        fns.insert(0, MFn.kPluginDependNode)
        # A plug-in's node is what its kind says, before what it is built on.
        chain = info["chain"]
        kind = _node_classes.get(chain[0], {}).get("kind") if chain else None
        base = chain[1] if len(chain) > 1 else ""
        if kind == 4:
            fns.insert(0, MFn.kPluginConstraintNode)
        elif kind == 6:
            fns.insert(0, MFn.kPluginIkSolver)
        elif base == "transform":
            fns.insert(0, MFn.kPluginTransformNode)
        elif base == "shape":
            fns.insert(0, MFn.kPluginShape)
        elif base == "locator":
            fns.insert(0, MFn.kPluginLocatorNode)
    return fns


class MObject:
    """A handle on a node, an attribute, a component or a data value."""

    __slots__ = ("_kind", "_id", "_attr", "_comp", "_data", "_fn")

    def __init__(self, src=None):
        self._kind = None
        self._id = 0
        self._attr = None
        self._comp = None
        self._data = None
        self._fn = None
        if isinstance(src, MObject):
            self._kind, self._id, self._attr, self._comp, self._data, self._fn = src._kind, src._id, src._attr, src._comp, src._data, src._fn

    def isNull(self):
        return self._kind is None

    def _fns(self):
        k = self._kind
        if k == "node":
            info = _api("node.info", id=self._id)
            return _fn_chain(info) if info else []
        if k == "world":
            return [MFn.kWorld]
        if k == "attr":
            return [self._fn, MFn.kAttribute]
        if k == "comp":
            return [self._fn] + _FN_BASES.get(self._fn, [MFn.kComponent])
        if k in ("data", "plugin"):
            return [self._fn]
        return []

    def apiType(self):
        if self._kind is None:
            return MFn.kInvalid
        fns = self._fns()
        return fns[0] if fns else MFn.kDependencyNode

    @property
    def apiTypeStr(self):
        return _FN_NAMES.get(self.apiType(), "kInvalid")

    def hasFn(self, fn):
        if fn in (MFn.kBase, MFn.kNamedObject):
            return self._kind is not None
        if fn == MFn.kDependencyNode:
            return self._kind == "node"
        return fn in self._fns()

    def __eq__(self, o):
        if not isinstance(o, MObject) or self._kind != o._kind:
            return False
        k = self._kind
        if k == "node":
            return self._id == o._id
        if k == "attr":
            return _attr_name(self) == _attr_name(o)
        if k == "comp":
            return self._fn == o._fn and self._comp == o._comp
        if k == "data":
            return self._data is o._data
        return True

    def __ne__(self, o):
        return not self == o

    def __hash__(self):
        return hash((self._kind, self._id, _attr_name(self) if self._kind == "attr" else None))

    def __repr__(self):
        return "<aztek.api.OpenAztek.MObject %s>" % (self.apiTypeStr,)


MObject.kNullObj = MObject()


def _node_object(id):
    o = MObject()
    o._kind, o._id = "node", id
    return o


def _world_object():
    o = MObject()
    o._kind = "world"
    return o


def _check_node(obj):
    if not isinstance(obj, MObject) or obj._kind != "node" or not _api("node.exists", id=obj._id):
        _fail()
    return obj._id


def _node_id_named(name):
    try:
        id = _api("node.find", name=name)
    except RuntimeError as e:
        _fail(message=str(e))
    return id


def _shape_below(id, type_name=None):
    """The first shape of a transform (intermediates last); the node itself when it is a shape."""
    info = _api("node.info", id=id)
    if not info or "transform" not in info["chain"]:
        return id
    later = None
    for c in info["children"]:
        k = _api("node.info", id=c)
        if "transform" in k["chain"] or not k["dag"] or (type_name and type_name not in k["chain"]):
            continue
        if not k["intermediate"]:
            return c
        later = later or c
    return later or id


class MObjectHandle:
    def __init__(self, obj=None):
        self._o = MObject(obj) if obj is not None else MObject()

    def assign(self, obj):
        self._o = MObject(obj)
        return self

    def object(self):
        return self._o

    def isAlive(self):
        return self._o._kind is not None and (self._o._kind != "node" or _api("node.exists", id=self._o._id))

    isValid = isAlive

    def hashCode(self):
        return hash(self._o) & 0xFFFFFFFF

    def __eq__(self, o):
        return isinstance(o, MObjectHandle) and self._o == o._o

    def __ne__(self, o):
        return not self == o

    __hash__ = None


class MDagPath:
    kInvalid = 0

    def __init__(self, src=None):
        self._nodes = list(src._nodes) if isinstance(src, MDagPath) else []

    @staticmethod
    def _of(id):
        p = MDagPath()
        chain = []
        while id:
            chain.append(id)
            id = _api("node.info", id=id)["parent"]
        p._nodes = chain[::-1]
        return p

    @staticmethod
    def getAPathTo(obj):
        id = _check_node(obj)
        if not _api("node.info", id=id)["dag"]:
            _fail()
        return MDagPath._of(id)

    @staticmethod
    def getAllPathsTo(obj):
        return MDagPathArray([MDagPath.getAPathTo(obj)])

    def _leaf(self):
        if not self.isValid():
            _fail(message="Object does not exist")
        return self._nodes[-1]

    def isValid(self):
        return bool(self._nodes) and all(_api("node.exists", id=i) for i in self._nodes[-1:])

    def fullPathName(self):
        return _api("node.info", id=self._leaf())["full"]

    def partialPathName(self):
        return _api("node.info", id=self._leaf())["path"]

    def node(self):
        return _node_object(self._leaf())

    def transform(self):
        for id in reversed(self._nodes):
            if "transform" in _api("node.info", id=id)["chain"]:
                return _node_object(id)
        _fail(message="Object does not exist")

    def apiType(self):
        return self.node().apiType()

    def hasFn(self, fn):
        return self.node().hasFn(fn)

    def length(self):
        return len(self._nodes)

    def isInstanced(self):
        return False

    def instanceNumber(self):
        return 0

    def getPath(self):
        return MDagPath(self)

    def set(self, other):
        self._nodes = list(other._nodes)
        return self

    def _matrices(self):
        return _api("dag.matrices", id=self._leaf())

    def inclusiveMatrix(self):
        return MMatrix(self._matrices()["world"])

    def exclusiveMatrix(self):
        return MMatrix(self._matrices()["parent"])

    def inclusiveMatrixInverse(self):
        return self.inclusiveMatrix().inverse()

    def exclusiveMatrixInverse(self):
        return self.exclusiveMatrix().inverse()

    def _shapes(self):
        out = []
        for c in _api("node.info", id=self._leaf())["children"]:
            k = _api("node.info", id=c)
            if "shape" in k["chain"]:
                out.append(c)
        return out

    def numberOfShapesDirectlyBelow(self):
        return len(self._shapes())

    def extendToShape(self, index=0):
        if "shape" in _api("node.info", id=self._leaf())["chain"]:
            return self
        shapes = self._shapes()
        if not shapes or index >= len(shapes):
            _fail(status="kFailure", message="Object has no shape below it")
        self._nodes.append(shapes[index])
        return self

    extendToShapeDirectlyBelow = extendToShape

    def childCount(self):
        return len(_api("node.info", id=self._leaf())["children"])

    def child(self, i):
        kids = _api("node.info", id=self._leaf())["children"]
        if i >= len(kids):
            raise IndexError("MDagPath child index out of range")
        return _node_object(kids[i])

    def push(self, obj):
        self._nodes.append(_check_node(obj))
        return self

    def pop(self, num=1):
        if num > len(self._nodes):
            _fail()
        del self._nodes[len(self._nodes) - num :]
        return self

    def isVisible(self):
        for id in self._nodes:
            n = _api("node.info", id=id)["path"]
            if not _cmds.getAttr(n + ".visibility"):
                return False
        return True

    def __eq__(self, o):
        # By where the paths lead now: a path follows its node through renames and reparenting.
        if not isinstance(o, MDagPath):
            return False
        if self._nodes == o._nodes:
            return True
        try:
            return self.fullPathName() == o.fullPathName()
        except RuntimeError:
            return False

    def __ne__(self, o):
        return not self == o

    __hash__ = None

    def __str__(self):
        return self.partialPathName() if self._nodes else ""

    def __repr__(self):
        return "aztek.api.OpenAztek.MDagPath(%s)" % (self.fullPathName() if self._nodes else "")


class MDagPathArray(_Array):
    _item = staticmethod(lambda x: MDagPath(x))
    _blank = staticmethod(MDagPath)


class MObjectArray(_Array):
    _item = staticmethod(lambda x: MObject(x))
    _blank = staticmethod(MObject)


def _attr_fn(type_name, long=""):
    if long in _ANGLE_ATTRS:
        return MFn.kUnitAttribute
    return {
        "enum": MFn.kEnumAttribute,
        "matrix": MFn.kMatrixAttribute,
        "string": MFn.kTypedAttribute,
        "mesh": MFn.kTypedAttribute,
        "data": MFn.kTypedAttribute,
        "vec3": MFn.kNumericAttribute,
    }.get(type_name, MFn.kNumericAttribute)


def _attr_object(node, long, type_name):
    o = MObject()
    o._kind = "attr"
    o._attr = long
    o._fn = _attr_fn(type_name, long)
    # The node it came from, so MFnAttribute can read the attribute's flags.
    o._data = node
    return o


def _split_plug(name):
    """(node id, attr path) of node.attr."""
    node, _, attr = name.partition(".")
    id = _node_id_named(node)
    if not id:
        _fail(message="Object does not exist")
    return id, attr


class MPlug:
    kAll = 0
    kNonDefault = 1
    kChanged = 2
    kLastAttrSelector = 3
    kFreeToChange = 0
    kNotFreeToChange = 1
    kChildrenNotFreeToChange = 2

    def __init__(self, a=None, b=None):
        self._node = 0
        self._attr = None
        if isinstance(a, MPlug):
            self._node, self._attr = a._node, a._attr
        elif a is not None:
            self._node = _check_node(a)
            self._attr = _attr_name(b)
            if self._attr is None:
                _fail()
            self._info()

    @staticmethod
    def _of(id, attr):
        p = MPlug()
        p._node, p._attr = id, attr
        return p

    @staticmethod
    def _named(name):
        return MPlug._of(*_split_plug(name))

    def _nodename(self):
        info = _api("node.info", id=self._node) if self._node else None
        if not info:
            _fail(message="Plug is not valid")
        return info["path"]

    def _host_attr(self):
        return self._attr

    def _hostname(self):
        return self._nodename() + "." + self._host_attr()

    def _info(self):
        try:
            d = _api("plug.info", name=self._hostname())
        except RuntimeError:
            _fail(message="Cannot find plug or attribute named '%s'" % (self._attr,))
        c = d["component"]
        if c >= 0:
            d["parent"] = d["long"]
            d["long"], d["short"] = d["children"][c], d["childShort"][c]
            d["children"], d["childShort"] = [], []
            d["type"] = "float"
        return d

    def _leaf_attr(self):
        return self._attr.rsplit("[", 1)[0] if self._attr.endswith("]") else self._attr

    def _is_one_element(self):
        return self._leaf_attr() in _ONE_ELEMENT

    def isNull(self):
        return not self._node or self._attr is None

    # Names

    def name(self):
        if self.isNull():
            return ""
        i = self._info()
        attr = self._attr if "." in self._attr or self._attr.endswith("]") else i["long"]
        return _api("node.info", id=i["node"])["path"] + "." + attr

    @property
    def info(self):
        return self.name()

    def partialName(self, includeNodeName=False, includeNonMandatoryIndices=False, includeInstancedIndices=False, useAlias=False, useFullAttributePath=False, useLongNames=False):
        i = self._info()
        base = i["long"] if useLongNames else (i["short"] or i["long"])
        if self._attr.endswith("]"):
            base += "[" + self._attr.rsplit("[", 1)[1]
        if includeNodeName:
            return _api("node.info", id=i["node"])["name"] + "." + base
        return base

    def node(self):
        return _node_object(self._info()["node"])

    def attribute(self):
        i = self._info()
        return _attr_object(i["node"], i["long"], i["type"])

    # Shape

    def isArray(self):
        if self._attr.endswith("]"):
            return False
        return self._info()["array"] or self._is_one_element()

    def isElement(self):
        return self._attr.endswith("]")

    def isCompound(self):
        return bool(self._info()["children"])

    def isChild(self):
        return "parent" in self._info()

    def numChildren(self):
        return len(self._info()["children"])

    def child(self, index):
        kids = self._info()["children"]
        if isinstance(index, MObject):
            n = _attr_name(index)
            for k, s in zip(kids, self._info()["childShort"]):
                if n in (k, s):
                    return MPlug._of(self._node, k)
            _fail(message="Attribute is not a child of this plug")
        if not 0 <= index < len(kids):
            raise IndexError("MPlug child index out of range")
        return MPlug._of(self._node, kids[index])

    def parent(self):
        p = self._info().get("parent")
        if not p:
            _fail(message="Plug is not a child")
        return MPlug._of(self._node, p)

    def array(self):
        if not self._attr.endswith("]"):
            _fail(message="Plug is not an array element")
        return MPlug._of(self._node, self._leaf_attr())

    def elementByLogicalIndex(self, index):
        if not self.isArray():
            _fail(message="Plug is not an array")
        return MPlug._of(self._node, "%s[%d]" % (self._attr, index))

    def _indices(self):
        if self._is_one_element():
            return [0]
        try:
            return _api("plug.elements", name=self._hostname())
        except RuntimeError:
            return []

    def elementByPhysicalIndex(self, index):
        ids = self._indices()
        if not 0 <= index < len(ids):
            raise IndexError("MPlug physical index out of range")
        return self.elementByLogicalIndex(ids[index])

    def numElements(self):
        if not self.isArray():
            _fail(message="Plug is not an array")
        return len(self._indices())

    evaluateNumElements = numElements

    def getExistingArrayAttributeIndices(self):
        return MIntArray(self._indices())

    def logicalIndex(self):
        if not self.isElement():
            _fail(message="Plug is not an array element")
        return int(self._attr.rsplit("[", 1)[1][:-1])

    # State

    def isConnected(self):
        c = _api("plug.connections", name=self._hostname())
        return bool(c["source"] or c["dests"])

    def isDestination(self):
        return bool(_api("plug.connections", name=self._hostname())["source"])

    def isSource(self):
        return bool(_api("plug.connections", name=self._hostname())["dests"])

    def isKeyable(self):
        return self._info()["keyable"]

    def isChannelBox(self):
        return self._info()["channelBox"]

    def isLocked(self):
        return self._info()["locked"]

    def isDynamic(self):
        return self._info()["user"]

    def isNetworked(self):
        return self.isConnected()

    def isFromReferencedFile(self):
        return False

    def isDefaultValue(self):
        v = self._value()
        return v == self._info()["default"]

    def setKeyable(self, on):
        _api("plug.setFlag", name=self._hostname(), flag="keyable", on=bool(on))
        return self

    def setChannelBox(self, on):
        _api("plug.setFlag", name=self._hostname(), flag="channelBox", on=bool(on))
        return self

    def setLocked(self, on):
        _cmds.setAttr(self._hostname(), lock=bool(on))
        return self

    # Connections

    def source(self):
        c = _api("plug.connections", name=self._hostname())
        return MPlug._named(c["source"]) if c["source"] else MPlug()

    sourceWithConversion = source

    def destinations(self):
        c = _api("plug.connections", name=self._hostname())
        return MPlugArray([MPlug._named(d) for d in c["dests"]])

    destinationsWithConversions = destinations

    def connectedTo(self, asDst, asSrc):
        c = _api("plug.connections", name=self._hostname())
        out = MPlugArray()
        if asDst and c["source"]:
            out.append(MPlug._named(c["source"]))
        if asSrc:
            for d in c["dests"]:
                out.append(MPlug._named(d))
        return out

    # Values

    def _value(self):
        v = _cmds.getAttr(self._hostname())
        if isinstance(v, list) and len(v) == 1 and isinstance(v[0], tuple):
            return list(v[0])
        return v

    def _is_angle(self):
        i = self._info()
        return i["long"] in _ANGLE_ATTRS or (self._node, i["long"]) in _PLUGIN_ANGLES or _plugin_attr_is_angle(self._node, i["long"])

    def _number(self):
        v = self._value()
        if isinstance(v, (list, str)) or v is None:
            _fail(message="Plug is not numeric")
        return v

    def asDouble(self, *a):
        v = float(self._number())
        return v

    def asFloat(self, *a):
        return _f32(self.asDouble())

    def asInt(self, *a):
        return int(self.asDouble())

    asShort = asInt
    asChar = asInt

    def asBool(self, *a):
        return bool(self._number())

    def asString(self, *a):
        v = self._value()
        if isinstance(v, str):
            return v
        if isinstance(v, (int, float)) and not isinstance(v, bool) and self._info()["type"] == "enum":
            names = self._info()["enum"]
            return names[int(v)] if 0 <= int(v) < len(names) else ""
        return "" if v is None else str(v)

    def asMAngle(self, *a):
        return MAngle(self.asDouble(), MAngle.kRadians)

    def asMDistance(self, *a):
        return MDistance(float(self._number()), MDistance.kCentimeters)

    def asMTime(self, *a):
        return MTime(float(self._number()))

    def asMObject(self, *a):
        i = self._info()
        v = self._value()
        o = MObject()
        o._kind = "data"
        o._data = v
        if i["type"] == "matrix":
            o._fn = MFn.kMatrixData
            o._data = MMatrix(v)
        elif i["type"] == "string":
            o._fn = MFn.kStringData
        elif i["type"] == "vec3":
            o._fn = MFn.kNumericData
        elif i["type"] in ("mesh", "data"):
            o._fn = MFn.kMeshData if i["type"] == "mesh" else MFn.kNumericData
        else:
            o._fn = MFn.kNumericData
        return o

    def _set(self, v):
        _cmds.setAttr(self._hostname(), v)
        return self

    def setDouble(self, v):
        return self._set(float(v))

    setFloat = setDouble

    def setInt(self, v):
        return self._set(int(v))

    setShort = setInt
    setChar = setInt

    def setBool(self, v):
        return self._set(bool(v))

    def setString(self, v):
        return self._set(str(v))

    def setMAngle(self, a):
        return self._set(a.asRadians())

    def setMDistance(self, d):
        return self._set(d.asCentimeters())

    def setMTime(self, t):
        return self._set(t.asUnits(MTime.uiUnit()))

    def setMObject(self, o):
        if o._kind != "data":
            _fail()
        d = o._data
        if isinstance(d, MMatrix):
            d = list(d)
        return self._set(d)

    def __eq__(self, o):
        if isinstance(o, MObject):
            # plug == attribute, as compute methods write it.
            n = _attr_name(o)
            if n is None or self.isNull():
                return False
            i = self._info()
            return n in (i["long"], i["short"])
        if not isinstance(o, MPlug) or self._node != o._node:
            return False
        if self._attr == o._attr:
            return True
        # tx and translateX are one plug.
        if self.isNull() or o.isNull():
            return False
        a, b = self._info(), o._info()
        return a["long"] == b["long"] and a["element"] == b["element"]

    def __ne__(self, o):
        return not self == o

    __hash__ = None

    def __str__(self):
        return self.name()

    def __repr__(self):
        return "aztek.api.OpenAztek.MPlug(%s)" % (self.name() if not self.isNull() else "")



class _Flag(int):
    """A plug state as API 2.0 gives it, a property (`plug.isChild`), that can still be called as API 1.0's method
    is (`plug.isChild()`)."""

    def __new__(cls, on):
        return int.__new__(cls, bool(on))

    def __call__(self):
        return bool(self)

    def __repr__(self):
        return repr(bool(self))

    __str__ = __repr__


def _flag_property(fn):
    return property(lambda self: _Flag(fn(self)), doc=fn.__doc__)


# The reference's API 2.0 has these as properties of MPlug, API 1.0 as methods.
for _n in ("isNull", "isArray", "isElement", "isCompound", "isChild", "isConnected", "isDestination", "isSource",
           "isKeyable", "isChannelBox", "isLocked", "isDynamic", "isNetworked", "isFromReferencedFile"):
    setattr(MPlug, _n, _flag_property(getattr(MPlug, _n)))
MPlug.isProcedural = property(lambda self: _Flag(False))
MPlug.isIgnoredWhenRendering = property(lambda self: _Flag(False))
MPlug.isCaching = property(lambda self: _Flag(False))
del _n

class MPlugArray(_Array):
    _item = staticmethod(lambda x: MPlug(x))
    _blank = staticmethod(MPlug)


# Plugin attributes in degrees, filled by om_plugin: (type name, attr).
_PLUGIN_ANGLES = set()


def _plugin_attr_is_angle(node, long):
    if not _PLUGIN_ANGLES:
        return False
    info = _api("node.info", id=node)
    return bool(info) and (info["type"], long) in _PLUGIN_ANGLES


def _attr_name(a):
    if isinstance(a, str):
        return a
    if isinstance(a, MObject) and a._kind == "attr":
        return a._attr if isinstance(a._attr, str) else a._attr["name"]
    return None


def _parse_ids(text):
    """Indices from 0:3,7 ; None for *."""
    out = []
    for part in text.split(","):
        part = part.strip()
        if part == "*":
            return None
        if ":" in part:
            a, b = part.split(":")
            out.extend(range(int(a), int(b) + 1))
        else:
            out.append(int(part))
    return out


def _compress_ids(ids):
    ids = sorted(set(ids))
    parts = []
    i = 0
    while i < len(ids):
        j = i
        while j + 1 < len(ids) and ids[j + 1] == ids[j] + 1:
            j += 1
        parts.append((ids[i], ids[j]))
        i = j + 1
    return parts


def _component_object(fn, ids):
    o = MObject()
    o._kind = "comp"
    o._fn = fn
    o._comp = list(ids)
    return o


def _component_count(shape, fn):
    if fn in (MFn.kCurveCVComponent, MFn.kCurveEPComponent):
        d = _api("curve.data", id=shape)
        return d["spans"] + (0 if d["form"] == 2 else d["degree"]) if fn == MFn.kCurveCVComponent else d["spans"] + 1
    m = _mesh_data(shape)
    return {
        MFn.kMeshVertComponent: m.num_vertices,
        MFn.kMeshEdgeComponent: m.num_edges,
        MFn.kMeshPolygonComponent: m.num_faces,
        MFn.kMeshMapComponent: m.num_uvs,
    }.get(fn, 0)


class _Item:
    """An entry of a selection list: a node, a plug or a node's components."""

    __slots__ = ("node", "plug", "comp")

    def __init__(self, node=0, plug=None, comp=None):
        self.node = node
        self.plug = plug
        self.comp = comp

    def same(self, o):
        if self.node != o.node:
            return False
        if self.plug is not None or o.plug is not None:
            return self.plug == o.plug
        if (self.comp is None) != (o.comp is None):
            return False
        return self.comp is None or (self.comp._fn == o.comp._fn and sorted(self.comp._comp) == sorted(o.comp._comp))

    def string(self):
        info = _api("node.info", id=self.node)
        if self.plug is not None:
            return self.plug.name()
        if self.comp is None:
            return info["path"]
        owner = info["path"]
        if info["parent"] and "shape" in info["chain"]:
            owner = _api("node.info", id=info["parent"])["path"]
        word = _COMP_NAMES[self.comp._fn]
        if self.comp._fn == MFn.kMeshVtxFaceComponent:
            return ["%s.%s[%d][%d]" % (owner, word, v, f) for v, f in self.comp._comp]
        return [
            "%s.%s[%s]" % (owner, word, str(a) if a == b else "%d:%d" % (a, b))
            for a, b in _compress_ids(self.comp._comp)
        ]


def _parse_item(text):
    """The items a string names: a component, a plug, or nodes (patterns allowed)."""
    m = _COMP_RE.match(text)
    if m:
        id = _node_id_named(m.group(1))
        if not id:
            _fail(message="Object does not exist")
        fn = _COMP_TYPES[m.group(2)]
        shape = _shape_below(id, "nurbsCurve" if m.group(2) in ("cv", "ep") else "mesh")
        if m.group(4) is not None:
            vs, fs = _parse_ids(m.group(3)), _parse_ids(m.group(4))
            return [_Item(shape, comp=_component_object(fn, [(v, f) for v in vs or [] for f in fs or []]))]
        ids = _parse_ids(m.group(3))
        if ids is None:
            ids = list(range(_component_count(shape, fn)))
        return [_Item(shape, comp=_component_object(fn, ids))]
    if "." in text:
        node, _, attr = text.partition(".")
        id = _node_id_named(node)
        if id:
            try:
                plug = MPlug._of(id, attr)
                plug._info()
                return [_Item(id, plug=plug)]
            except RuntimeError:
                pass
        _fail(message="Object does not exist")
    if "*" in text or "?" in text:
        found = _cmds.ls(text)
        if not found:
            _fail(message="Object does not exist")
        return [_Item(_node_id_named(n)) for n in found]
    id = None
    try:
        id = _api("node.find", name=text)
    except RuntimeError as e:
        _fail(message=str(e))
    if not id:
        _fail(message="Object does not exist")
    return [_Item(id)]


class MSelectionList:
    kMergeNormal = 0
    kXORWithList = 1
    kRemoveFromList = 2

    def __init__(self, src=None):
        self._items = list(src._items) if isinstance(src, MSelectionList) else []

    def _at(self, i):
        if not 0 <= i < len(self._items):
            raise IndexError("MSelectionList index out of range")
        return self._items[i]

    def _add_item(self, item, merge):
        for e in self._items:
            if e.same(item):
                return
            if merge and e.comp is not None and item.comp is not None and e.node == item.node and e.comp._fn == item.comp._fn:
                e.comp._comp = sorted(set(e.comp._comp) | set(item.comp._comp))
                return
        self._items.append(item)

    def _items_of(self, item):
        if isinstance(item, str):
            return _parse_item(item)
        if isinstance(item, MObject):
            return [_Item(_check_node(item))]
        if isinstance(item, MDagPath):
            return [_Item(item._leaf())]
        if isinstance(item, MPlug):
            return [_Item(item._node, plug=MPlug(item))]
        if isinstance(item, (tuple, list)) and len(item) == 2:
            path, comp = item
            leaf = path._leaf() if isinstance(path, MDagPath) else _check_node(path)
            if comp is None or comp.isNull():
                return [_Item(leaf)]
            return [_Item(_shape_below(leaf), comp=_component_object(comp._fn, comp._comp))]
        _fail()

    def add(self, item, mergeWithExisting=True):
        for i in self._items_of(item):
            self._add_item(i, mergeWithExisting)
        return self

    def length(self):
        return len(self._items)

    def __len__(self):
        return len(self._items)

    def isEmpty(self):
        return not self._items

    def clear(self):
        self._items = []
        return self

    def merge(self, *args):
        """merge(list, strategy) or merge(dagPath, component, strategy)."""
        strategy = MSelectionList.kMergeNormal
        if isinstance(args[0], MSelectionList):
            items = list(args[0]._items)
            if len(args) > 1:
                strategy = args[1]
        else:
            items = self._items_of((args[0], args[1] if len(args) > 1 else None))
            if len(args) > 2:
                strategy = args[2]
        for it in items:
            found = [e for e in self._items if e.same(it)]
            if strategy == MSelectionList.kRemoveFromList or (strategy == MSelectionList.kXORWithList and found):
                self._items = [e for e in self._items if not e.same(it)]
            elif not found:
                self._add_item(_Item(it.node, it.plug, it.comp), True)
        return self

    def remove(self, index):
        self._at(index)
        del self._items[index]
        return self

    def replace(self, index, item):
        self._at(index)
        self._items[index] = self._items_of(item)[0]
        return self

    def toggle(self, path, component=None):
        return self.merge(path, component, MSelectionList.kXORWithList)

    def hasItem(self, item):
        its = self._items_of(item)
        return all(any(e.same(i) for e in self._items) for i in its)

    def hasItemPartly(self, path, component):
        its = self._items_of((path, component))
        for i in its:
            for e in self._items:
                if e.node == i.node and e.comp is not None and e.comp._fn == i.comp._fn and set(e.comp._comp) & set(i.comp._comp):
                    return True
        return False

    def getDagPath(self, index):
        it = self._at(index)
        if not _api("node.info", id=it.node)["dag"]:
            _fail()
        return MDagPath._of(it.node)

    def getDependNode(self, index):
        return _node_object(self._at(index).node)

    def getPlug(self, index):
        it = self._at(index)
        if it.plug is None:
            _fail()
        return MPlug(it.plug)

    def getComponent(self, index):
        it = self._at(index)
        if not _api("node.info", id=it.node)["dag"]:
            _fail()
        path = MDagPath._of(it.node)
        return (path, _component_object(it.comp._fn, it.comp._comp) if it.comp is not None else MObject())

    def getSelectionStrings(self, index=None):
        items = self._items if index is None else [self._at(index)]
        out = []
        for it in items:
            s = it.string()
            out.extend(s if isinstance(s, list) else [s])
        return out

    def copy(self, src):
        self._items = list(src._items)
        return self


class MGlobal:
    kReplaceList = 0
    kXORWithList = 1
    kAddToList = 2
    kRemoveFromList = 3
    kAddToHeadOfList = 4
    kInteractive = 0
    kBatch = 1
    kLibraryApp = 2
    kBaseUIMode = 3
    kSelectObjectMode = 0
    kSelectComponentMode = 1

    @staticmethod
    def mayaState():
        return MGlobal.kInteractive

    @staticmethod
    def mayaVersion():
        return "2024"

    @staticmethod
    def apiVersion():
        return 20240000

    @staticmethod
    def getActiveSelectionList(orderedSelectionIfAvailable=False):
        out = MSelectionList()
        for s in _cmds.ls(sl=True) or []:
            out.add(s)
        return out

    @staticmethod
    def setActiveSelectionList(sel, listAdjustment=0):
        names = sel.getSelectionStrings()
        MGlobal._select(names, listAdjustment)

    @staticmethod
    def _select(names, mode):
        if mode == MGlobal.kReplaceList:
            # Replacing with objects leaves component mode, or the components stay selected as well.
            if not any(_COMP_RE.match(n) for n in names):
                _cmds.selectMode("object")
            if names:
                _cmds.select(names)
            else:
                _cmds.select(clear=True)
        elif names:
            if mode == MGlobal.kXORWithList:
                _cmds.select(names, toggle=True)
            elif mode == MGlobal.kRemoveFromList:
                _cmds.select(names, deselect=True)
            else:
                _cmds.select(names, add=True)

    @staticmethod
    def selectByName(pattern, listAdjustment=0):
        found = _cmds.ls(pattern)
        if not found:
            _fail(message="Object does not exist")
        MGlobal._select(found, listAdjustment)

    @staticmethod
    def select(obj, comp=None, listAdjustment=0):
        s = MSelectionList()
        if isinstance(obj, MSelectionList):
            s = obj
        elif comp is not None:
            s.add((obj, comp))
        else:
            s.add(obj)
        MGlobal._select(s.getSelectionStrings(), listAdjustment)

    @staticmethod
    def clearSelectionList():
        _cmds.select(clear=True)

    @staticmethod
    def getSelectionListByName(pattern):
        s = MSelectionList()
        s.add(pattern)
        return s

    @staticmethod
    def displayInfo(msg):
        _az.write(str(msg), 3)

    @staticmethod
    def displayWarning(msg):
        _az.write("# Warning: %s #" % (msg,), 4)

    @staticmethod
    def displayError(msg):
        _az.write("# Error: %s #" % (msg,), 2)

    @staticmethod
    def _mel(command):
        v = _json.loads(_az.mel(command))
        if isinstance(v, dict) and "__error__" in v:
            raise RuntimeError(v["__error__"])
        return v

    @staticmethod
    def executeCommand(command, displayEnabled=False, undoEnabled=False):
        MGlobal._mel(command)

    @staticmethod
    def executeCommandStringResult(command, displayEnabled=False, undoEnabled=False):
        v = MGlobal._mel(command)
        if isinstance(v, list):
            return [str(x) for x in v]
        return "" if v is None else str(v)

    @staticmethod
    def executeCommandOnIdle(command, displayEnabled=False):
        _az.mel(command)

    @staticmethod
    def executePythonCommand(command, displayEnabled=False, undoEnabled=False):
        import __main__

        g = __main__.__dict__
        try:
            code = compile(command, "<executePythonCommand>", "eval")
        except SyntaxError:
            exec(compile(command, "<executePythonCommand>", "exec"), g)
            return None
        return eval(code, g)

    @staticmethod
    def executePythonCommandStringResult(command, displayEnabled=False, undoEnabled=False):
        v = MGlobal.executePythonCommand(command)
        return "" if v is None else str(v)

    @staticmethod
    def executePythonCommandOnIdle(command, displayEnabled=False):
        MGlobal.executePythonCommand(command)

    @staticmethod
    def isUndoing():
        return False

    @staticmethod
    def isRedoing():
        return False

    @staticmethod
    def isYAxisUp():
        return _cmds.upAxis(q=True, axis=True) == "y"

    @staticmethod
    def isZAxisUp():
        return _cmds.upAxis(q=True, axis=True) == "z"

    @staticmethod
    def upAxis():
        return MVector(0, 0, 1) if MGlobal.isZAxisUp() else MVector(0, 1, 0)

    @staticmethod
    def setYAxisUp(rotateView=False):
        _cmds.upAxis(axis="y", rotateView=bool(rotateView))

    @staticmethod
    def setZAxisUp(rotateView=False):
        _cmds.upAxis(axis="z", rotateView=bool(rotateView))

    @staticmethod
    def getFunctionSetList(obj):
        return [_FN_NAMES.get(f, "kBase") for f in obj._fns()] + ["kBase"]


class MDGModifier:
    """Edits queued until doIt, undone in reverse by undoIt. Nodes are created at once, as the reference does, and go again on undoIt."""

    def __init__(self):
        self._ops = []
        self._done = 0

    def _queue(self, do, undo):
        self._ops.append((do, undo))

    def doIt(self):
        for do, _ in self._ops[self._done :]:
            do()
        self._done = len(self._ops)
        return self

    def undoIt(self):
        for _, undo in reversed(self._ops[: self._done]):
            undo()
        self._done = 0
        return self

    def _create(self, type_name, parent=None):
        if isinstance(type_name, MTypeId):
            type_name = _node_type_of_id(type_name)
        kw = {"skipSelect": True}
        if parent is not None:
            kw["parent"] = _api("node.info", id=parent)["path"]
        made = _cmds.createNode(type_name, **kw)
        obj = _node_object(_node_id_named(made))
        state = {"exists": True}

        def again():
            if not state["exists"]:
                obj._id = _node_id_named(_cmds.createNode(type_name, **kw))
                state["exists"] = True

        def gone():
            if state["exists"]:
                _cmds.delete(_api("node.info", id=obj._id)["path"])
                state["exists"] = False

        self._queue(again, gone)
        return obj

    def createNode(self, type_name):
        return self._create(type_name)

    def _plugs(self, args):
        if len(args) == 2:
            return args[0], args[1]
        if len(args) == 4:
            return MPlug(args[0], args[1]), MPlug(args[2], args[3])
        _fail()

    def connect(self, *args):
        src, dst = self._plugs(args)
        s, d = src._hostname(), dst._hostname()
        self._queue(lambda: _cmds.connectAttr(s, d), lambda: _cmds.disconnectAttr(s, d))
        return self

    def disconnect(self, *args):
        src, dst = self._plugs(args)
        s, d = src._hostname(), dst._hostname()
        self._queue(lambda: _cmds.disconnectAttr(s, d), lambda: _cmds.connectAttr(s, d))
        return self

    def deleteNode(self, node, includeParents=False):
        id = _check_node(node)
        name = _api("node.info", id=id)["path"]
        info = _api("node.info", id=id)
        kind, parent = info["type"], info["parent"]

        def undo():
            kw = {"skipSelect": True, "name": info["name"]}
            if parent:
                kw["parent"] = _api("node.info", id=parent)["path"]
            node._id = _node_id_named(_cmds.createNode(kind, **kw))

        self._queue(lambda: _cmds.delete(_api("node.info", id=node._id)["path"] if node._id else name), undo)
        return self

    def renameNode(self, node, newName):
        id = _check_node(node)
        old = _api("node.info", id=id)["name"]
        self._queue(lambda: _cmds.rename(_api("node.info", id=node._id)["path"], newName), lambda: _cmds.rename(_api("node.info", id=node._id)["path"], old))
        return self

    def newPlugValue(self, plug, data):
        return self._plug_value(plug, data._data)

    def _plug_value(self, plug, v):
        p = MPlug(plug)
        old = p._value()
        self._queue(lambda: p._set(v), lambda: p._set(old))
        return self

    def newPlugValueBool(self, plug, v):
        return self._plug_value(plug, bool(v))

    def newPlugValueInt(self, plug, v):
        return self._plug_value(plug, int(v))

    newPlugValueShort = newPlugValueInt
    newPlugValueChar = newPlugValueInt

    def newPlugValueDouble(self, plug, v):
        return self._plug_value(plug, float(v))

    newPlugValueFloat = newPlugValueDouble

    def newPlugValueString(self, plug, v):
        return self._plug_value(plug, str(v))

    def newPlugValueMAngle(self, plug, a):
        return self._plug_value(plug, a.asRadians())

    def newPlugValueMDistance(self, plug, d):
        return self._plug_value(plug, d.asCentimeters())

    def newPlugValueMTime(self, plug, t):
        return self._plug_value(plug, t.asUnits(MTime.uiUnit()))

    def addAttribute(self, node, attr):
        def add():
            MFnDependencyNode(node).addAttribute(attr)

        def rem():
            MFnDependencyNode(node).removeAttribute(attr)

        self._queue(add, rem)
        return self

    def removeAttribute(self, node, attr):
        def rem():
            MFnDependencyNode(node).removeAttribute(attr)

        self._queue(rem, lambda: MFnDependencyNode(node).addAttribute(attr))
        return self

    def commandToExecute(self, command):
        self._queue(lambda: _az.mel(command), lambda: None)
        return self

    def pythonCommandToExecute(self, command):
        self._queue(lambda: MGlobal.executePythonCommand(command), lambda: None)
        return self


class MDagModifier(MDGModifier):
    def createNode(self, type_name, parent=None):
        if isinstance(type_name, MTypeId):
            type_name = _node_type_of_id(type_name)
        parent_id = None
        if parent is not None and not parent.isNull():
            parent_id = None if parent._kind == "world" else _check_node(parent)
        return self._create(type_name, parent_id)

    def reparentNode(self, node, newParent=None):
        id = _check_node(node)
        was = _api("node.info", id=id)["parent"]

        def to(target):
            path = _api("node.info", id=node._id)["path"]
            if target:
                _cmds.parent(path, _api("node.info", id=target)["path"])
            else:
                _cmds.parent(path, world=True)

        target = None if newParent is None or newParent.isNull() or newParent._kind == "world" else _check_node(newParent)
        self._queue(lambda: to(target), lambda: to(was))
        return self
