# 术语

## 声明式（Declarative）

用「状态对应怎样的界面」描述 UI，而不是逐步下达绘制指令。框架/运行时负责把状态变化反映到树上。本项目的主写法。

## Retained（保留式）

运行时**长期持有**控件/节点树。节点有身份与生命周期；状态变化时做脏区/局部更新，不必每帧从零描述整屏。本项目的主运行时模型。

## Immediate（立即模式）

每一帧（或每次刷新）用代码把「当前屏上该有什么」说一遍；框架不默认长期记住完整控件对象树。概念形如：

```text
每帧:
  if Button("保存") { ... }
  Slider(&速度)
```

Dear ImGui、egui 等属于此类思路。本项目中仅作**可选视口**（局部或整窗），需显式声明。

## GPU 加速

用 GPU 执行合批、纹理、合成等绘制。优化的是绘制成本，不自动等于更少逻辑重绘。与 Immediate / retained 正交：两种模型都可以接 GPU。

## 软渲染（Software rasterization）

CPU 完成栅格化，不依赖独显。Field 档的重要兜底，与 GPU 后端共用场景图。

## 脏区（Dirty / damage region）

只重绘变化涉及的区域，减少无效刷新。retained 天然好做；纯 Immediate 要做等价事往往要引入缓存，从而靠近 retained。

## Field / Office Profile

同一内核上的两套部署档位：现场弱机稳优先；办公室 GPU 与动效预算更高。见 [architecture.md](architecture.md)。

## 场景图（Scene graph）

retained 侧持有的节点/绘制结构；GPU 与软渲染都从这里取可绘制表示。
