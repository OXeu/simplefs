# Exfs

**WIP...**

## 特性

- [x] 灵活多级索引，最高支持 256 级索引
- [x] 区间索引
- [x] 无单文件大小限制（理论最小文件大小上限$ 256^255 * 4 KB = 4.9*10 ^611 GB = 4.8 * 10 ^ 608 TB $）
- [x] 文件块缓存

## 文件结构

```mermaid
graph LR
    sb[SuperBlock]
    b[Bitmap]
    d[AllocatedDataBlocks]
    f[Free Blocks]
    i[Inode];
    d --- f;
    sb --- b --- d;
    f --- i
```

### 现有问题

#### Bitmap

Inode不是一个块只有一个，Bitmap按照一个块进行映射是不行的
Inode 一个块可以放 64 条

#### vim打开文件总是提示有交换文件

#### 文件大于 4KB 读取被截断