# Features covered compared to the C++ Feather engine

What feather.rs has of the C++ engine (`guitarprompter/contrib/feather`), and more; the missing
features are in [../todo/missing-features-vs-cpp.md](../todo/missing-features-vs-cpp.md).

Features feather.rs already covers (not listed below): scene graph with transforms and visibility,
materials with uniforms, uniform inheritance down the hierarchy, PNG atlases with metadata,
textures as atlas regions, cuboid / plane / OBJ mesh builders, text in the XY and XZ planes
(one mesh or one node per glyph), transparency sorting back to front, directional shadow maps,
named lookup of resources, animations (animator, slide and perpetual animations, easing
functions), mouse input, the `featherui` UI library (`feather-ui`: hud, NSWE layout, spacer,
scrollbars; plus labels and buttons, which C++ didn't have), Android (assets read from the APK,
logging to logcat, the app's lifecycle: with Vulkan only the window's surface and swapchain are
lost when the app goes to the background, not every GPU resource as with the C++ engine's
OpenGL context; touch as the mouse; see [../android.md](../android.md)), binary meshes (`.fmesh`
files, made from OBJs by the mesh converter, which replaces `ObjMeshConverter`, loaded with the
loader picked by the file's extension; see [../fmesh-format.md](../fmesh-format.md)), the atlas
creator and font creator tools.
feather.rs also has things C++ never had (Vulkan, SPIR-V reflection, sphere builder, skeletons
and animation clips, JSON scene files, cameras as scene objects (perspective or orthographic,
several per scene with an active one, defined in scene files), several scenes per frame).
