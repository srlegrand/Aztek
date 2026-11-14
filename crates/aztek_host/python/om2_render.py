# aztek.api.OpenAztekRender (maya.api.OpenMayaRender): the reference's 2.0 viewport module. Thin (om_thin): plug-ins
# that register draw overrides load, and asking for a redraw works; drawing through these classes is not in Aztek yet.


class MRenderer(_Thin):
    """The viewport renderer. Asking for a redraw works; the rest raises when called."""

    @staticmethod
    def theRenderer(initializeRenderer=True):
        return MRenderer()

    @staticmethod
    def setGeometryDrawDirty(obj, topologyChanged=True):
        # Aztek redraws what changed on its own.
        return None

    @staticmethod
    def setLightsAndShadowsDirty():
        return None

    @staticmethod
    def needEvaluateAllLights():
        return None

    @staticmethod
    def disableChangeManagementUntilNextRefresh():
        return None


class MDrawRegistry(_Thin):
    """Draw overrides by classification. Kept, not used: Aztek draws plug-in nodes its own way."""

    _registered = {}

    @staticmethod
    def _keep(kind, classification, registrantId, creator=None):
        MDrawRegistry._registered[(kind, classification, registrantId)] = creator
        return None

    @staticmethod
    def _drop(kind, classification, registrantId):
        MDrawRegistry._registered.pop((kind, classification, registrantId), None)
        return None


for _kind in (
    "DrawOverride", "GeometryOverride", "SubSceneOverride", "ShaderOverride", "ShadingNodeOverride",
    "SurfaceShadingNodeOverride", "VertexBufferGenerator", "VertexBufferMutator", "IndexBufferMutator",
    "PrimitiveGenerator", "ComponentConverter", "ImagePlaneOverride", "ShaderOverrideCreator",
):
    setattr(
        MDrawRegistry, "register" + _kind + "Creator",
        staticmethod((lambda k: lambda classification, registrantId, creator=None, *a: MDrawRegistry._keep(k, classification, registrantId, creator))(_kind)),
    )
    setattr(
        MDrawRegistry, "deregister" + _kind + "Creator",
        staticmethod((lambda k: lambda classification, registrantId, *a: MDrawRegistry._drop(k, classification, registrantId))(_kind)),
    )
del _kind


_thin_classes(
    [
        "MAttributeParameterMapping", "MAttributeParameterMappingList", "MBlendState", "MBlendStateDesc", "MCameraOverride",
        "MClearOperation", "MColorManagementUtilities", "MComponentDataIndexing", "MComponentDataIndexingList",
        "MDepthNormalizationDescription", "MDepthStencilState", "MDepthStencilStateDesc", "MDrawContext",
        "MFragmentManager", "MFrameContext", "MGeometry", "MGeometryExtractor", "MGeometryIndexMapping",
        "MGeometryRequirements", "MGeometryUtilities", "MHUDRender", "MIndexBuffer", "MIndexBufferDescriptor",
        "MIndexBufferDescriptorList", "MInitContext", "MInitFeedback", "MIntersection", "MLightParameterInformation",
        "MPassContext", "MPresentTarget", "MPreviewRenderEnvironment", "MPxComponentConverter", "MPxDrawOverride",
        "MPxGeometryOverride", "MPxImagePlaneOverride", "MPxIndexBufferMutator", "MPxPrimitiveGenerator",
        "MPxShaderOverride", "MPxShadingNodeOverride", "MPxSubSceneOverride", "MPxSurfaceShadingNodeOverride",
        "MPxVertexBufferGenerator", "MPxVertexBufferMutator", "MQuadRender", "MRasterizerState", "MRasterizerStateDesc",
        "MRenderItem", "MRenderItemList", "MRenderOperation", "MRenderOverride", "MRenderParameters", "MRenderProfile",
        "MRenderTarget", "MRenderTargetAssignment", "MRenderTargetDescription", "MRenderTargetManager",
        "MRenderUtilities", "MSamplerState", "MSamplerStateDesc", "MSceneRender", "MSelectionContext", "MSelectionInfo",
        "MShaderCompileMacro", "MShaderInstance", "MShaderManager", "MStateManager", "MStencilOpDesc",
        "MSubSceneContainer", "MSubSceneContainerIterator", "MSwatchRenderBase", "MTargetBlendDesc", "MTexture",
        "MTextureAssignment", "MTextureDescription", "MTextureManager", "MTextureUpdateRegion", "MUIDrawManager",
        "MUniformParameter", "MUniformParameterList", "MUserRenderOperation", "MVaryingParameter",
        "MVaryingParameterList", "MVertexBuffer", "MVertexBufferArray", "MVertexBufferDescriptor",
        "MVertexBufferDescriptorList",
    ],
    globals(),
)
