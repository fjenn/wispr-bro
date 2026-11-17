# Notice: code ported from Hex

Parts of `ios-native/` are ported from **Hex** (https://github.com/kitlangton/hex),
directory `ios/`, under the MIT License. Each ported file names its source in a
header comment.

| Wispr Bro file | Hex source |
| --- | --- |
| `WBShared/WBBridge.swift`, `WBShared/WBHostApps.swift` | `ios/HEXShared/KeyboardBridge.swift` |
| `WBShared/WBKeyboardSurface.swift` | `ios/HEXShared/KeyboardSurface.swift` |
| `WBKeyboard/WBKeyboardViewController.swift` | `ios/HEXKeyboard/KeyboardViewController.swift` |
| `WBKeyboard/WBContainingAppOpener.swift` | `ios/HEXKeyboard/ContainingAppOpener.swift` |
| `WBSession/WBAudioEngine.swift` | `ios/HEXMobile/AudioBackend.swift` |
| `WBSession/WBSessionController.swift` | `ios/HEXMobile/DictationController.swift` |
| `WBSession/WBLiveActivityCoordinator.swift` | `ios/HEXMobile/LiveActivityCoordinator.swift` |
| `WBSession/WBSwipeBackHint.swift` | `ios/HEXShared/HomeSwipeCue.swift` (idea; redrawn in SwiftUI) |
| `WBActivity/WBActivityAttributes.swift` | `ios/HEXActivity/HexActivityAttributes.swift` |
| `WBWidgets/WBSessionLiveActivity.swift` | `ios/HEXWidgets/HexDictationLiveActivity.swift` |
| `Harness/WBHarnessApp.swift` (preview stage) | `ios/HEXMobile/KeyboardPreviewHost.swift` (idea) |
| `project.yml` | `ios/project.yml` |

Modifications (c) the Wispr Bro contributors, GPL-3.0-or-later.

```
MIT License

Copyright (c) 2026 Christopher Langton

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
