# Chusan Common 背景与前景 SrPlayer 的编辑器合成边界

本文记录 `CommonBackGroundObject` 与 `AdvertiseLogoObject` 同时存在时，当前二进制能够证明的对象建立顺序，以及编辑器据此实现的显式双 SRD 预览边界。它不把某个 Common SRD 文件名或当前游戏模式自动选择规则冒充成二进制结论。

## ObjectFactory 列表

`sub_C13110` 构造 `projView::ObjectFactory` 的对象列表。每一项先把名称指针与零组成 8 字节 pair，再调用 `sub_406271 -> sub_C1AE80` 追加到 vector。`sub_C1AE80` 只扩容并按原样复制这两个 dword，不排序：

- `0xC1357C` 读取 `off_1C1D500`，其字符串为 `CommonBackGroundObject`；`0xC13591` 完成该 pair 的追加；
- `0xC13702` 读取 `off_1C20370`，其字符串为 `AdvertiseLogoObject`；`0xC13717` 完成该 pair 的追加。

同一构造函数稍后把名称与工厂对应起来：

- `0xC146D5` 以 `off_1C1D500` 构造名称参数，随后调用 Common 工厂注册入口；
- `0xC14998` 以 `off_1C20370` 构造名称参数，随后调用 Advertise 工厂注册入口。

因此这两个名称不是相似字符串推断，而是实际对象列表和实际工厂表使用的同一静态名称对象。

## 创建遍历保持列表顺序

`ObjectManager::createAllObject` (`sub_B2D580`) 取得上述 vector 的 begin/end，以 8 字节步长从前向后遍历，并把每项 `+0x04` 的名称指针交给 `sub_B2D690`。`sub_B2D690` 按名称查找对应工厂、创建对象并插入 manager。该链没有反转或排序，所以对象建立调用中 Common 位于 Advertise 之前。

这项结论与两个 SrPlayer 已分别闭环的宿主状态一致：Common 使用 `2DLayer=6`、root key `0x8680`；Advertise 使用 `2DLayer=100`、root key `0xE480`。不过，当前证据还没有把所有游戏模式的对象 Enable 切换、每帧 SrPlayer 调用点和其他同时存在对象穷尽到一条通用“自动场景栈”算法。因此 Rust 不会根据前景文件名自动猜测应加载哪一个 Common SRD。

## 编辑器实现

Properties 的 `COMMON BACKGROUND LOWER LAYER` 接受一个用户显式选择的第二 SRD、SCN、ANMS 和 frame。该层固定使用已经证明的 `CommonBackGroundObject` / selected target host profile，前景继续使用 `AdvertiseLogoObject` profile。Composition 只清屏一次，随后先提交 Common 的完整 Image/Slice/Number/Fennel target stream，再提交前景 stream。

两个文件分别拥有：

- 自己的 TEXL/CROP 与 DDS GPU 资源；
- 自己的 RFZ FontManager slot、atlas texture token 路由；
- 自己的 reference runtime、ANMS frame、runtime draw 和 merged target command。

Fennel token 只在所属玩家的 atlas route 中解释，不跨文件混用。D3D9Ex `ResetEx` 会同时失效并重建两组 DEFAULT-pool 纹理/atlas 资源。

SCN Composition 尺寸必须完全相等。若不同，编辑器直接报错；当前二进制没有提供一条可以安全外推为编辑器自动缩放的规则。

命令行等价入口为：

```powershell
cargo run -- "D:\sdhd\assets\data\surfboard\advertise\CHU_UI_Advertise_00_v10.srd" --srd-runtime-smoke=0,0,24 --advertise-logo-host=MainScene@1080x1920@1920x1080 --common-background-layer="D:\sdhd\assets\data\surfboard\common\commonBackGround\CHU_UI_Common_BK_00_v11.srd"
```

该回归中 Common `yellow_loop` 单独改变全部 2,073,600 个 RGB 像素；Advertise 当前 warning 页面随后完全遮住背景，所以最终双层结果与仅前景的 RGB 差分为 0。这不是背景未提交：smoke 分别回读 Common-only、foreground-only 和双层目标，并在强制 `ResetEx` 前后得到相同结果。Advertise 的 Fennel 差分仍为 73,588 像素。

## 仍未闭环

- 游戏当前模式如何选择具体 Common SRD 与 ANMS；
- Common、Advertise 之外所有同时启用 SrPlayer 的每帧调用顺序；
- 构造完成之后 target Enable/current-target 与玩家 Enable 的完整时序；
- 不同 SCN Composition 尺寸之间是否存在某个具体宿主的显式变换。

这些输入闭环前，双层预览保持用户显式选择，不提供自动猜测。
