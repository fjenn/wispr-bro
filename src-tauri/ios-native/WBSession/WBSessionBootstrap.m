// SPDX-License-Identifier: GPL-3.0-or-later
// Starts the session controller without any change to the app's main():
// +load runs when the binary is loaded and waits for the launch notification.
// The Swift class is found by name, so this file needs no generated header.

#import <UIKit/UIKit.h>

@interface WBSessionBootstrap : NSObject
@end

@implementation WBSessionBootstrap

+ (void)load {
    [[NSNotificationCenter defaultCenter]
        addObserverForName:UIApplicationDidFinishLaunchingNotification
                    object:nil
                     queue:[NSOperationQueue mainQueue]
                usingBlock:^(NSNotification *note) {
        Class controller = NSClassFromString(@"WBSessionController");
        SEL selector = NSSelectorFromString(@"bootstrapWithLaunchURL:");
        if (![controller respondsToSelector:selector]) {
            return;
        }
        NSURL *url = note.userInfo[UIApplicationLaunchOptionsURLKey];
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Warc-performSelector-leaks"
        [controller performSelector:selector withObject:url];
#pragma clang diagnostic pop
    }];
}

@end
