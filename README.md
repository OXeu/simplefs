# SimpleFS
## 初始化
```shell
fs init <size>
```
\<size>: 文件系统大小，单位为MB

## 文件系统组织形式
每 4KB 为一个文件块，
// 文件夹是一个 4KB 的文件，内部存储子文件索引