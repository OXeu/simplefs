<h1 align="center"> Exfs </h1>
<p align="center"> Extensive FileSystem </p>

## 特性

- [x] 多级索引，可退化为直接索引，最高支持 256 级索引
- [x] 区间索引
- [x] 无单文件大小限制（理论最小文件大小上限 $256^ {255} * 4\space KB = 4.9*10 ^{611}\space GB = 4.8 * 10 ^{608}\space
  TB$
- [x] 文件块缓存

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