[![Build Status](https://dev.azure.com/zvezda0002/Aiur/_apis/build/status/vzvezda.aiur?branchName=master)](https://dev.azure.com/zvezda0002/Aiur/_build/latest?definitionId=2&branchName=master)

# aiur

Aiur is async executor for Rust that has been created to explorer how far we can go with the following initial design ideas:

* structured concurrency 
* single thread executor
* lifetime bounded 
* no or minimal dynamic allocations

It currently under the heavy development in not yet ready for production use.

## Usage

aiur itself is only an "executor" part of the async runtime, which is in a nutshell a task management and channels. To write a program it usually needed a "reactor" component, which is code that makes actual I/O with OS. aiur has a "toy runtime" built-in, e.g. reactor that can only do waitings. 

aiur is not supposed to be directly used by the apps. Some reactor library should be built upon aiur and apps can use runtime that provided by this reactor library. There is no such reactor library are published yet.

Since aiur does not have any system I/O it is based on rust standard library, so it is portable. 



