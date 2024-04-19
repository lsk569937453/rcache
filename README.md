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
![alt tag](https://raw.githubusercontent.com/lsk569937453/image_repo/main/rcache/c.png)

总共做了三组对比实验:

- redis:原生 redis
- rcache(mpsc 的 channel 版本):rcache 的 1.0 实现，代码位于tag 0.0.1版本
- rcache(全局 mutex 版本):rcache 参考 mini-redis 实现,性能和 mini-redis 一样，代码位于主分支代码

实验结果
rcache 基于 mini-redis 的实现，性能等于 mini-redis。相比原生的 redis ，单线程吞吐量是 redis 的 90%，多线程的吞吐量是原生的 redis 的两倍。

# 架构
参考 mini-redis,直接用 Mutex 对全局的数据加锁,全局的 struct 内部其实就是用多个 HashMap 来存储 string,list,hash 以及过期 map 等数据结构。没想到效果出奇的好。


