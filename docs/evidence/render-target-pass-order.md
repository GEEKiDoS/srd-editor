# Target pass queue 与最终遍历顺序

本文继续 [`render-target-routing.md`](render-target-routing.md) 中 target-local 分类之后的路径，只记录 `chusanApp.exe` 已经由指令流闭环的队列行为。分析对象 SHA-256 仍为 `28EBB4580A4CAE8ED0605B37F2F7C16460497412FE352E020A43D3A082FFEB67`。

## ScenePassModule 只分类，不排序

基础 target 命令处理 `sub_601DE0` 在完成 target-local camera/depth/visibility 计算后，经 thunk `sub_40F538` 调用 `sub_64BAB0`。`sea::ScenePassModule` 构造函数 `sub_64B530` 证明：

- `ScenePassModule+0x10` 指向 `scene+0x170` 的 `sea::SceneModelModule`；
- `ScenePassModule+0x18..+0x20` 是步长 `0x18` 的 rule vector；
- rule vector 的构造 reserve 数为 128。

`sub_64BAB0` 从 rule vector 首地址开始前向扫描，按 packet 分类值、rule mode 和 depth/u16 threshold 选择第一个匹配项。匹配后以该 rule 的数组下标调用 `SceneModelModule` 虚表 `+0x08`。这里没有比较器或重排；depth 只参与 rule 条件，并可写回 command `+0x30`。

## SceneModelModule 的 per-pass 容器

`sea::SceneModelModule` 构造函数 `sub_648C00` 建立两个 swap buffer。其虚表 `0x181BFEC` 已闭环为：

| 槽 | 落点 | 行为 |
| ---: | --- | --- |
| `+0x08` | `sub_6499A0` | 把 command 路由到 rule-index pass |
| `+0x0C` | `sub_649FF0` | 前向执行当前 buffer 的 20 字节资源/状态命令 |
| `+0x10` | `sub_649C20` | 返回指定 pass 的 `0x38` command 数量 |
| `+0x14` | `sub_649B20` | 前向绘制指定 pass |
| `+0x18` | `sub_649AE0` | 把每个 pass 的 end 恢复到 begin |

每个 pass rule 在当前 buffer 中占 `0x14` 字节，其首部是一个保存 `0x38` command 的 vector。`sub_6499A0` 的普通分支直接在 `20 * rule_index` 处调用 `sub_6041C0`。`sub_6041C0` 只做 vector 扩容、原位 `0x38` 拷贝和 end 指针递增；已入队记录不会被交换、反转或排序。

普通/特殊分支由 command `+0x10 bit 0x4` 决定。SRD 的已证明初始链为：

1. `ceylon_construct_draw_packet` (`0x6B8B10`) 把 packet `+0x84` 的低字节清零；
2. `srd_construct_renderer` (`0xAC4010`) 在同字段 OR `0x10`；
3. `ceylon_submit_vertex_batch` (`0x6DF020`) 在同字段 OR `0x01`；
4. packet `+0x80..+0x8F` 被原位复制到 command `+0x0C..+0x1B`，所以 command `+0x10` 的低字节为 `0x11`，不含 `0x04`。

因此当前 SRD quad 的这条提交链进入普通 per-pass vector，而不是 special environment-cache 分支。special 分支仍存在，但不能用来解释默认 SRD 顺序。

## target 以 32 个 EntryInfo 范围消费 pass

基础 scene 在 `scene+0x210` 构造的 target render module 虚表 `+0x04` 落到 `sub_64EE80`。帧尾 `sub_603850` 的顺序为：

1. 调用 `SceneModelModule+0x0C` 执行资源/状态命令；
2. 把 `SceneModelModule`、target width 和 height 传给 `scene+0x210` 虚表 `+0x04`；
3. 调用 `SceneModelModule+0x18` 清空当前 pass end 指针。

`sub_64EE80` 读取当前 swap buffer 的 32 个 `sea::EntryInfo`。每个 entry 保存一对 signed pass index `(first, last)`：

- `first < 0`：该 entry 不绘制；
- `first > last`：不绘制；
- 其余情况：从 `first` 到 `last` 按递增顺序逐个调用 `SceneModelModule+0x14`；
- `SceneModelModule+0x14` 对越界 pass index 直接返回，对有效 pass 则从 vector begin 到 end 以 `0x38` 步长前向提交。

所以最终 target-local 顺序不是 comparator sort，而是：

```text
初始 command 顺序
  -> first-matching rule index
  -> 每个 rule/pass 内稳定前向追加
  -> 32 个 EntryInfo 的数组顺序
  -> 每个 EntryInfo 的 [first, last] 递增 pass 顺序
  -> 每个 pass 内稳定前向提交
```

EntryInfo 范围允许重复或重叠；二进制会再次遍历同一个 pass，因此不能擅自去重。

Rust 的 `target_pass::build_evidence_scene_model_submission_indices` 已复现这一段已经闭环的稳定分桶与 32-entry inclusive-range 遍历。它要求调用者提供已经分类出的 rule index 和明确的 target EntryInfo，不负责猜测 target profile。

## 仍未闭合的输入

队列容器、稳定性和 flush 遍历已经闭环，但独立 SRD 文件仍不足以生成唯一最终 GPU 列表。还需继续证明：

- `sub_443095` 对全部 SRD packet 状态产生的分类值；
- Chusan 实际 `MainScene`、`BgScene`、common background 宿主及其他 target 在当前模式下安装的 `0x18` rule records；
- 对应 target 当前 swap buffer 的 32 个 EntryInfo `(first,last)`；
- target filter 的激活集合和切换时序；
- `ceylon_enqueue_draw_packet` 在相邻 packet 状态相同情况下的 vertex-range 合并如何映射到编辑器的逻辑 draw 项。

在这些 target-specific 输入闭环前，Rust planner 不能自行分配 pass index，也不能把 RefCast 的 CAST 调用序列直接提交给 D3D9Ex。
