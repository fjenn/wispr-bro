// SPDX-License-Identifier: GPL-3.0-or-later
//
// Wispr Bro for iPhone: the UIKit scene life cycle for a tao/Tauri app.
//
// An app built with the iOS 27 SDK must use the scene-based life cycle
// (UIKit aborts at launch otherwise: "UIScene life cycle is required for
// apps built with this SDK"). tao (the windowing layer under Tauri) still
// uses the application-delegate life cycle: it makes its UIWindow without a
// scene and listens to the application delegate's callbacks. This file
// bridges the two without touching tao:
//
//  - Info.ios.plist names WBSceneDelegate as the one window scene's delegate;
//  - a window created without a scene (tao's) is given the app's scene;
//  - the scene's life-cycle and open-URL callbacks are passed on to the
//    application delegate's old ones, which tao and Tauri handle, and a URL
//    is also posted as WBOpenURLNotification (object: the NSURL) for the
//    native session code.
//
// Compiled into the Rust library by build.rs; `wb_ios_scene_install` is
// called from `fork::ios::start` before UIKit starts.

#import <UIKit/UIKit.h>
#import <objc/message.h>
#import <objc/runtime.h>

NSNotificationName const WBOpenURLNotification = @"WBOpenURLNotification";

// 0 in front, 1 inactive, 2 in the background (fork::ios::host::app_state);
// kept from UIKit's notifications so any thread may ask.
static _Atomic int gAppState = 1;
int wb_ios_app_state(void) { return gAppState; }

// fork::ios::memory: unloads the cleanup model (the speech model stays).
extern void wb_ios_memory_warning(void);

static UIWindowScene *gScene;
static NSHashTable<UIWindow *> *gWindows;

// The app's background (the launch screen's colour, light and dark), so
// nothing white shows between the launch screen and the page's first paint.
static UIColor *WBBackground(void) {
    UIColor *named = [UIColor colorNamed:@"LaunchBackground"];
    return named ?: UIColor.systemBackgroundColor;
}

static void WBPaint(UIView *view, UIColor *color) {
    if ([view isKindOfClass:NSClassFromString(@"WKWebView")]) {
        view.opaque = NO;
        view.backgroundColor = color;
        UIScrollView *scroll = [view valueForKey:@"scrollView"];
        if ([scroll isKindOfClass:UIScrollView.class]) {
            scroll.backgroundColor = color;
        }
        return;
    }
    for (UIView *child in view.subviews) {
        WBPaint(child, color);
    }
}

static void WBPaintWindow(UIWindow *window) {
    UIColor *color = WBBackground();
    window.backgroundColor = color;
    window.rootViewController.view.backgroundColor = color;
    WBPaint(window, color);
}

static UIView *WBFindWebView(UIView *view) {
    if ([view isKindOfClass:NSClassFromString(@"WKWebView")]) {
        return view;
    }
    for (UIView *child in view.subviews) {
        UIView *found = WBFindWebView(child);
        if (found != nil) {
            return found;
        }
    }
    return nil;
}

// The keyboard's height over the page, told to the page before the
// keyboard moves (`window.__wbKeyboardInset(points)`, useKeyboardInset.ts):
// the page lays itself out above the keyboard in step with it. The web
// view's own viewport events come late and not every time.
static void WBKeyboardChanged(NSNotification *note) {
    CGFloat covered = 0;
    UIWindow *window = nil;
    for (UIWindow *candidate in gWindows.allObjects) {
        if (WBFindWebView(candidate) != nil) {
            window = candidate;
            break;
        }
    }
    if (window == nil) {
        return;
    }
    if (![note.name isEqualToString:UIKeyboardWillHideNotification]) {
        NSValue *value = note.userInfo[UIKeyboardFrameEndUserInfoKey];
        if (value != nil) {
            CGRect frame = [window convertRect:value.CGRectValue fromWindow:nil];
            covered = MAX(0, CGRectGetMaxY(window.bounds) - CGRectGetMinY(frame));
        }
    }
    UIView *webView = WBFindWebView(window);
    SEL evaluate = NSSelectorFromString(@"evaluateJavaScript:completionHandler:");
    if ([webView respondsToSelector:evaluate]) {
        NSString *script = [NSString
            stringWithFormat:@"window.__wbKeyboardInset&&window.__wbKeyboardInset(%.0f)", covered];
        ((void (*)(id, SEL, NSString *, id))objc_msgSend)(webView, evaluate, script, nil);
    }
}

static void WBAdopt(UIWindow *window) {
    WBPaintWindow(window);
    if (gScene == nil || window.windowScene != nil) {
        return;
    }
    window.windowScene = gScene;
    if (!window.hidden) {
        [window makeKeyAndVisible];
    }
}

@interface UIWindow (WBScene)
@end

@implementation UIWindow (WBScene)

// Swapped with -initWithFrame: (so the call below runs the original).
- (instancetype)wb_initWithFrame:(CGRect)frame {
    UIWindow *window = [self wb_initWithFrame:frame];
    if (window != nil) {
        [gWindows addObject:window];
        // After the caller finished setting the window up; a window UIKit
        // made for a scene has its scene by then and is left alone.
        dispatch_async(dispatch_get_main_queue(), ^{
            WBAdopt(window);
        });
    }
    return window;
}

@end

@interface WBSceneDelegate : UIResponder <UIWindowSceneDelegate>
@end

// One opened URL, on the main thread.
static void WBHandleURL(NSURL *url) {
    // A session link from the keyboard (wisprbro://…) goes to the Swift
    // session controller, when it is in the app (looked up by name: the
    // Rust library builds without it).
    Class controller = NSClassFromString(@"WBSessionController");
    SEL shared = NSSelectorFromString(@"shared");
    SEL handle = NSSelectorFromString(@"handleOpenWithUrl:");
    if ([controller respondsToSelector:shared]) {
        id session = ((id (*)(id, SEL))objc_msgSend)(controller, shared);
        if ([session respondsToSelector:handle] &&
            ((BOOL (*)(id, SEL, NSURL *))objc_msgSend)(session, handle, url)) {
            NSLog(@"WBScene: session link handled by the session controller");
            return;
        }
    }
    NSLog(@"WBScene: URL passed to the application delegate (scheme %@)", url.scheme);
    id<UIApplicationDelegate> delegate = UIApplication.sharedApplication.delegate;
    if ([delegate respondsToSelector:@selector(application:openURL:options:)]) {
        [delegate application:UIApplication.sharedApplication openURL:url options:@{}];
    }
    [NSNotificationCenter.defaultCenter postNotificationName:WBOpenURLNotification object:url];
}

// The same handling for a URL given as text (the self-test: the simulator
// asks "Open in Wispr Bro?" for a link opened with simctl).
void wb_ios_scene_open_url(const char *text) {
    NSString *string = text ? [NSString stringWithUTF8String:text] : nil;
    NSURL *url = string ? [NSURL URLWithString:string] : nil;
    if (url == nil) {
        return;
    }
    dispatch_async(dispatch_get_main_queue(), ^{
        WBHandleURL(url);
    });
}

@implementation WBSceneDelegate

- (id<UIApplicationDelegate>)appDelegate {
    return UIApplication.sharedApplication.delegate;
}

- (void)openURLContexts:(NSSet<UIOpenURLContext *> *)contexts {
    for (UIOpenURLContext *context in contexts) {
        if (NSProcessInfo.processInfo.environment[@"WB_DEBUG_HOST_OBSERVER"] != nil) {
            NSLog(@"WBScene: link opened by %@", context.options.sourceApplication);
        }
        WBHandleURL(context.URL);
    }
}

- (void)scene:(UIScene *)scene
    willConnectToSession:(UISceneSession *)session
                 options:(UISceneConnectionOptions *)connectionOptions {
    if (![scene isKindOfClass:UIWindowScene.class]) {
        return;
    }
    gScene = (UIWindowScene *)scene;
    for (UIWindow *window in gWindows.allObjects) {
        WBAdopt(window);
    }
    // The web view is added to the window a little after the window is made.
    for (NSNumber *delay in @[@0.0, @0.05, @0.2, @0.6]) {
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(delay.doubleValue * NSEC_PER_SEC)),
                       dispatch_get_main_queue(), ^{
            for (UIWindow *window in gWindows.allObjects) {
                WBPaintWindow(window);
            }
        });
    }
    if (connectionOptions.URLContexts.count > 0) {
        NSSet<UIOpenURLContext *> *contexts = connectionOptions.URLContexts;
        // After launch finished, so every observer is in place.
        dispatch_async(dispatch_get_main_queue(), ^{
            [self openURLContexts:contexts];
        });
    }
}

- (void)sceneDidDisconnect:(UIScene *)scene {
    if (scene == gScene) {
        gScene = nil;
    }
}

- (void)scene:(UIScene *)scene openURLContexts:(NSSet<UIOpenURLContext *> *)URLContexts {
    [self openURLContexts:URLContexts];
}

- (void)sceneDidBecomeActive:(UIScene *)scene {
    id<UIApplicationDelegate> delegate = [self appDelegate];
    if ([delegate respondsToSelector:@selector(applicationDidBecomeActive:)]) {
        [delegate applicationDidBecomeActive:UIApplication.sharedApplication];
    }
}

- (void)sceneWillResignActive:(UIScene *)scene {
    id<UIApplicationDelegate> delegate = [self appDelegate];
    if ([delegate respondsToSelector:@selector(applicationWillResignActive:)]) {
        [delegate applicationWillResignActive:UIApplication.sharedApplication];
    }
}

- (void)sceneWillEnterForeground:(UIScene *)scene {
    id<UIApplicationDelegate> delegate = [self appDelegate];
    if ([delegate respondsToSelector:@selector(applicationWillEnterForeground:)]) {
        [delegate applicationWillEnterForeground:UIApplication.sharedApplication];
    }
}

- (void)sceneDidEnterBackground:(UIScene *)scene {
    id<UIApplicationDelegate> delegate = [self appDelegate];
    if ([delegate respondsToSelector:@selector(applicationDidEnterBackground:)]) {
        [delegate applicationDidEnterBackground:UIApplication.sharedApplication];
    }
}

@end

void wb_ios_scene_install(void) {
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        gWindows = [NSHashTable weakObjectsHashTable];
        Method original = class_getInstanceMethod(UIWindow.class, @selector(initWithFrame:));
        Method ours = class_getInstanceMethod(UIWindow.class, @selector(wb_initWithFrame:));
        if (original != NULL && ours != NULL) {
            method_exchangeImplementations(original, ours);
        }
        for (NSNotificationName name in @[
                 UIKeyboardWillChangeFrameNotification, UIKeyboardWillHideNotification
             ]) {
            [NSNotificationCenter.defaultCenter addObserverForName:name
                                                            object:nil
                                                             queue:NSOperationQueue.mainQueue
                                                        usingBlock:^(NSNotification *note) {
                WBKeyboardChanged(note);
            }];
        }
        NSDictionary<NSNotificationName, NSNumber *> *states = @{
            UIApplicationDidBecomeActiveNotification : @0,
            UIApplicationWillResignActiveNotification : @1,
            UIApplicationDidEnterBackgroundNotification : @2,
            UIApplicationWillEnterForegroundNotification : @1,
        };
        for (NSNotificationName name in states) {
            int state = states[name].intValue;
            [NSNotificationCenter.defaultCenter addObserverForName:name
                                                            object:nil
                                                             queue:nil
                                                        usingBlock:^(__unused NSNotification *note) {
                gAppState = state;
            }];
        }
        // A memory warning: the cleanup model goes first (Rust decides).
        [NSNotificationCenter.defaultCenter
            addObserverForName:UIApplicationDidReceiveMemoryWarningNotification
                        object:nil
                         queue:nil
                    usingBlock:^(__unused NSNotification *note) {
            wb_ios_memory_warning();
        }];
        // No "previous / next / done" bar over the keyboard: it is a web
        // form control, not something an iPhone app shows. The web view's
        // content view (WKContentView, looked up by name) answers nil for
        // its input accessory view. Every hop is checked; when one is
        // missing the bar simply stays.
        Class content = NSClassFromString(@"WKContentView");
        Method accessory = content != Nil
            ? class_getInstanceMethod(content, @selector(inputAccessoryView))
            : NULL;
        if (accessory != NULL) {
            method_setImplementation(accessory, imp_implementationWithBlock(^id(id view) {
                return nil;
            }));
        }
    });
}
