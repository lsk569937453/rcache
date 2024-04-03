# Rcache
Rcache是基于rust实现的redis缓存。该项目是一个demo工程，主要演示了如何使用rust实现redis的核心模块。

# 支持的指令
- set
- get
- append
- decr
- getdel
- getrange
- getset
- incr
- incrby
- incrbyfloat
- lcs
- mget
- mset
- msetnx
- lpush
- rpush
- lpop
- rpop
- sadd
- hset
- zadd
- lrange

# 压测结果
通过redis-benchmark对rcache和redis进行性能对比(4核心8G内存)测试，结果如下:
![alt tag](https://raw.githubusercontent.com/lsk569937453/image_repo/main/rcache/yidongbangong20240403102041.png)

redis-benchmark使用多线程(添加--threads 16参数)参数进行性能测试，rcache和redis的性能基本持平。
redis-benchmark使用单线程(添加--threads 1参数)参数进行性能测试，rcache的性能大概是redis的80%左右。

# 架构分析
rcache的网络请求是多协程处理的，解析完网络请求后，将请求放入mpsc管道，由统一的协程进行处理。

