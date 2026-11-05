// SPDX-License-Identifier: GPL-3.0-or-later
//
// RS-03 display helper (test tool only, never shipped).
//
//   vdisplay plug W H SCALE SECONDS  add a virtual display (CGVirtualDisplay,
//                                    private CoreGraphics class; the same
//                                    route DeskPad and BetterDisplay use) and
//                                    keep it until killed or SECONDS pass;
//                                    exiting is the "unplug".
//   vdisplay mode1x SECONDS          switch the main display to a 1x mode for
//                                    this process only (kCGConfigureForAppOnly
//                                    reverts when the process exits).
//
// The class declarations follow DeskPad's `CGVirtualDisplayPrivate.h`
// (MIT, Copyright (c) 2022 Bastian Andelefski), re-typed from its public
// interface.

#import <CoreGraphics/CoreGraphics.h>
#import <Foundation/Foundation.h>

@interface CGVirtualDisplayDescriptor : NSObject
@property(retain, nonatomic) dispatch_queue_t queue;
@property(retain, nonatomic) NSString *name;
@property(nonatomic) unsigned int maxPixelsHigh;
@property(nonatomic) unsigned int maxPixelsWide;
@property(nonatomic) CGSize sizeInMillimeters;
@property(nonatomic) unsigned int serialNum;
@property(nonatomic) unsigned int productID;
@property(nonatomic) unsigned int vendorID;
@property(copy, nonatomic) void (^terminationHandler)(id, id);
@end

@interface CGVirtualDisplayMode : NSObject
- (instancetype)initWithWidth:(unsigned int)width
                       height:(unsigned int)height
                  refreshRate:(double)refreshRate;
@end

@interface CGVirtualDisplaySettings : NSObject
@property(retain, nonatomic) NSArray *modes;
@property(nonatomic) unsigned int hiDPI;
@end

@interface CGVirtualDisplay : NSObject
@property(readonly, nonatomic) unsigned int displayID;
- (instancetype)initWithDescriptor:(CGVirtualDisplayDescriptor *)descriptor;
- (BOOL)applySettings:(CGVirtualDisplaySettings *)settings;
@end

static int plug(unsigned w, unsigned h, unsigned scale, double seconds) {
  CGVirtualDisplayDescriptor *d = [[CGVirtualDisplayDescriptor alloc] init];
  d.queue = dispatch_get_main_queue();
  d.name = @"RS-03 virtual display";
  d.maxPixelsWide = w * scale;
  d.maxPixelsHigh = h * scale;
  d.sizeInMillimeters = CGSizeMake(600, 340);
  d.serialNum = 0x5203;
  d.productID = 0x5203;
  d.vendorID = 0x5203;
  CGVirtualDisplay *vd = [[CGVirtualDisplay alloc] initWithDescriptor:d];
  if (!vd) {
    fprintf(stderr, "vdisplay: create failed\n");
    return 1;
  }
  CGVirtualDisplaySettings *s = [[CGVirtualDisplaySettings alloc] init];
  s.hiDPI = scale > 1 ? 1 : 0;
  s.modes = @[ [[CGVirtualDisplayMode alloc] initWithWidth:w * scale
                                                   height:h * scale
                                              refreshRate:60] ];
  if (![vd applySettings:s]) {
    fprintf(stderr, "vdisplay: applySettings failed\n");
    return 1;
  }
  printf("ready %u\n", vd.displayID);
  fflush(stdout);
  [[NSRunLoop mainRunLoop]
      runUntilDate:[NSDate dateWithTimeIntervalSinceNow:seconds]];
  return 0;
}

static int mode1x(double seconds) {
  CGDirectDisplayID main = CGMainDisplayID();
  NSDictionary *opt = @{(id)kCGDisplayShowDuplicateLowResolutionModes : @YES};
  CFArrayRef modes = CGDisplayCopyAllDisplayModes(main, (CFDictionaryRef)opt);
  CGDisplayModeRef pick = NULL;
  size_t best = 0;
  for (CFIndex i = 0; i < CFArrayGetCount(modes); i++) {
    CGDisplayModeRef m = (CGDisplayModeRef)CFArrayGetValueAtIndex(modes, i);
    size_t w = CGDisplayModeGetWidth(m), pw = CGDisplayModeGetPixelWidth(m);
    if (pw == w && w >= 1280 && w <= 1800 && w > best &&
        CGDisplayModeIsUsableForDesktopGUI(m)) {
      best = w;
      pick = m;
    }
  }
  if (!pick) {
    fprintf(stderr, "vdisplay: no 1x mode\n");
    return 1;
  }
  CGDisplayConfigRef cfg;
  CGBeginDisplayConfiguration(&cfg);
  CGConfigureDisplayWithDisplayMode(cfg, main, pick, NULL);
  CGError e = CGCompleteDisplayConfiguration(cfg, kCGConfigureForAppOnly);
  printf("mode1x %zux%zu err=%d\n", CGDisplayModeGetWidth(pick),
         CGDisplayModeGetHeight(pick), e);
  fflush(stdout);
  [[NSRunLoop mainRunLoop]
      runUntilDate:[NSDate dateWithTimeIntervalSinceNow:seconds]];
  CFRelease(modes);
  return e == kCGErrorSuccess ? 0 : 1;
}

int main(int argc, const char **argv) {
  @autoreleasepool {
    if (argc >= 6 && strcmp(argv[1], "plug") == 0)
      return plug(atoi(argv[2]), atoi(argv[3]), atoi(argv[4]), atof(argv[5]));
    if (argc >= 3 && strcmp(argv[1], "mode1x") == 0)
      return mode1x(atof(argv[2]));
    fprintf(stderr, "usage: vdisplay plug W H SCALE SECONDS | mode1x SECONDS\n");
    return 2;
  }
}
