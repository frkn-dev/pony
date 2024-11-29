# cmake-ts

A CMake-based build system for native NodeJS and Electron addons.

This project is loosely inspired by [cmake-js](https://github.com/cmake-js/cmake-js) but attempts to fix several design flaws.

It is intended to prebuild addons for different versions of NodeJS and Electron and ship a binary version.

## Configuration

Configuration is done entirely via `package.json`. You can specify multiple build configurations under the `cmake-ts` key:

```js
"cmake-ts": {
  "nodeAPI": "node-addon-api" // Specify the node API package such as `node-addon-api`, `nan`, or the path to a directory that has the nodeAPI header. Default is `node-addon-api`, a warning is emitted if nan is used
  "configurations": [
    {
      "name": "win-x64", // name for named-configs mode
      "os": "win32", // win32, linux and darwin are supported
      "arch": "x64", // x64, x86 should work
      "runtime": "electron", // node or electron
      "runtimeVersion": "4.0.1", // Version of the runtime which it is built
      "toolchainFile": "/windows.cmake", // CMake Toolchain file to use for crosscompiling
      "CMakeOptions": [ //Same syntax as for the globalCMakeOptions
        {
          "name": "MY_CMAKE_OPTION",
          "value": "my_value",
        }
      ],
      "addonSubdirectory": "avx2-generic" // if you build addons for multiple architectures in high performance scenarios, you can put the addon inside another subdirectory
    }, // more build configurations...
    {
      "dev": true, // whether this configuration is eligible to be used in a dev test build
      "os": "linux", // win32, linux and darwin are supported
      "arch": "x64", // x64, x86 should work
      "runtime": "node", // node or electron
      "runtimeVersion": "10.3.0", // Version of the runtime which it is built
    } // more build configurations ...
  ],
  "targetDirectory": "build", // where to build your project
  "buildType": "Release", // Debug or Release build, most likely set it to Release
  "projectName": "addon" // The name of your CMake project.
  "globalCMakeOptions": [{ // this might be omitted of no further options should be passed to CMake
    "name": "CMAKE_CXX_FLAGS",
    "value": "-Og"
  }, {
    "name": "CMAKE_CXX_FLAGS",
    "value": "-I$ROOT$/include", // $ROOT$ will be replaced by the package.json directory
  }, {
      "name": "CMAKE_EXPORT_COMPILE_COMMANDS",
      "value": "1"
  }]
}
```

## Workflow

While it is desirable to perform a full build (all configurations) within a CI environment, long build times hinder local package development. Therefore cmake-ts knows multiple build modes:

- **TODO** `nativeonly` -> Builds the native code **only** for the runtime cmake-ts is currently running on, ignoring all previously specified configurations. This is useful if you'd like to run some unit tests against the compiled code. When running `cmake-ts nativeonly`, cmake-ts will determine the runtime, ABI, and platform from the environment, and build only the configuration required to run on this platform.
  - *Example using the configuration above*
  - You run `cmake-ts nativeonly` on **NodeJS 11.7 on MacOS**, `cmake-ts` will **ignore** all specified configurations above and build the native addon for **NodeJS 11.7 on MacOS**
- **TODO** `osonly` -> Builds the native code for all configurations which match the current operating system. This is useful for those developing for example an electron addon and want to test their code in electron. In such a case, you would specify electron and NodeJS runtimes for several platforms in your configuration and you can use `cmake-ts osonly` to build a local package you can install in your application.
  - *Example using the configuration above*
  - You run `cmake-ts osonly` on **NodeJS 11.7 on Linux**, `cmake-ts` will **ignore** all configurations above where `os != linux` and build the native addon for **all** remaining configurations, in this case it will build for **NodeJS 10.3 on Linux**.
- **TODO** **HINT**: For both `osonly` and `nativeonly`, the specified CMake Toolchain files are ignored since I assume you got your toolchain set up correctly for your **own** operating system.
- None / Omitted: Builds all configs
- `dev-os-only` builds the first config that has `dev == true` and `os` matches the current OS
- `named-configs arg1 arg2 ...` builds all configs for which `name` is one of the args

## Cross Compilation

This module supports cross-compilation from Linux to macOS and Windows, given a correct toolchain setup. There is a docker container that has a cross-toolchain based on CLang 7 setup for Windows and macOS which might be used in a CI.

[Docker Image](https://hub.docker.com/r/martin31821/crossdev)
