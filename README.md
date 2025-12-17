# RustyCore

This is a WIP library to interface with the MacOS CoreBluetooth Library in Rust as BOTH the Client (Central) and the Server (Peripheral). 

Resources used in development: 
[CoreBluetooth](https://developer.apple.com/library/archive/documentation/NetworkingInternetWeb/Conceptual/CoreBluetooth_concepts/AboutCoreBluetooth/Introduction.html#//apple_ref/doc/uid/TP40013257-CH1-SW1RL)

This code is heavily inspired by:
[Ble-Peripheral](https://github.com/rohitsangwan01/ble-peripheral-rust/tree/main)
[btleplug](https://github.com/deviceplug/btleplug/tree/master)


Since this library is specific for interacting with MacOS CoreBluetooth and this interface abstracts away the hardware adapter layer.
I.e 
Does not give us the tight grain of control where we can select our adapters and decide if we want to use it as a Central or Peripheral. 
Manager
  └── Adapter(s)          
        └── Central API    
        └── Peripheral API

We do not need to have Manager and Adapter abstractions we can just use Central and Peripheral.
