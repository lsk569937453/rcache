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
# rdb持久化时间统计

```
rcache           | Rdb file has been saved,keys count is 19623,encode time cost 60ms,total time cost 60ms
rcache           | Rdb file has been saved,keys count is 227031,encode time cost 683ms,total time cost 683ms
rcache           | Rdb file has been saved,keys count is 436068,encode time cost 1322ms,total time cost 1322ms
rcache           | Rdb file has been saved,keys count is 642909,encode time cost 1951ms,total time cost 1951ms
rcache           | Rdb file has been saved,keys count is 849693,encode time cost 2579ms,total time cost 2579ms
rcache           | Rdb file has been saved,keys count is 1053635,encode time cost 3319ms,total time cost 3319ms
rcache           | Rdb file has been saved,keys count is 1256806,encode time cost 3946ms,total time cost 3946ms
rcache           | Rdb file has been saved,keys count is 1460143,encode time cost 4439ms,total time cost 4439ms
rcache           | Rdb file has been saved,keys count is 1662464,encode time cost 5153ms,total time cost 5153ms
rcache           | Rdb file has been saved,keys count is 1849100,encode time cost 6048ms,total time cost 6048ms
rcache           | Rdb file has been saved,keys count is 2045408,encode time cost 6958ms,total time cost 6958ms
rcache           | Rdb file has been saved,keys count is 2235381,encode time cost 7189ms,total time cost 7189ms
rcache           | Rdb file has been saved,keys count is 2431964,encode time cost 7577ms,total time cost 7577ms
rcache           | Rdb file has been saved,keys count is 2629802,encode time cost 8172ms,total time cost 8172ms
rcache           | Rdb file has been saved,keys count is 2827623,encode time cost 8854ms,total time cost 8854ms
rcache           | Rdb file has been saved,keys count is 3023288,encode time cost 9540ms,total time cost 9540ms
rcache           | Rdb file has been saved,keys count is 3216428,encode time cost 10372ms,total time cost 10372ms
rcache           | Rdb file has been saved,keys count is 3409558,encode time cost 6106ms,total time cost 6106ms
rcache           | Rdb file has been saved,keys count is 3519240,encode time cost 10828ms,total time cost 10828ms
```
可以看出rdb的时间随着key的多少而不同，300w个key的持久化时间大概在9s左右。
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


