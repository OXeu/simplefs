<h1 align="center"> VAFS </h1>
<p align="center"> Vast FileSystem </p>

## 特性

- [x] 多级索引，可退化为直接索引，理论最高支持 256 级索引，64 ZB文件最多使用 8 级索引
- [x] 区间索引
- [x] 单文件大小上限(寻址上限) $2\space^{64} * 4\space KB = 64 \space ZB$
- [x] LRU 文件块缓存

## 文件结构

```mermaid
graph LR
    sb[SuperBlock]
    ib[InodeBitmap]
    b[DataBitmap]
    i[InodeBlocks];
    d[DataBlocks]
    sb --- ib --- b --- i --- d  
```
