import AppKit
import CoreGraphics
import Foundation
func pixels(_ file:String)->(Int,Int,[UInt8]) {
 let source=NSImage(contentsOfFile:file)!.cgImage(forProposedRect:nil,context:nil,hints:nil)!
 let w=source.width,h=source.height
 var bytes=[UInt8](repeating:0,count:w*h*4)
 bytes.withUnsafeMutableBytes { storage in
  let context=CGContext(data:storage.baseAddress!,width:w,height:h,bitsPerComponent:8,bytesPerRow:w*4,space:CGColorSpaceCreateDeviceRGB(),bitmapInfo:CGImageAlphaInfo.premultipliedLast.rawValue)!
  context.draw(source,in:CGRect(x:0,y:0,width:w,height:h))
 }
 return(w,h,bytes)
}
var results:[String:Any]=[:]
for theme in ["dark","light"] {for mode in ["dialog-unlock","dialog-cancel","note-apply","pk-apply","delete-apply","delete-cancel","insert-apply","insert-cancel"] {
 let base="/tmp/next-gui-view-keys-v1-native-keys/"+theme+"-"+mode+"/"
 let(w,h,before)=pixels(base+"before.png")
 if (mode.hasPrefix("dialog") || mode.hasPrefix("delete") || mode.hasPrefix("insert")) {let(bw,bh,blocked)=pixels(base+"blocked-save.png");precondition(w==bw && h==bh);precondition(before==blocked,"Save shortcut bypassed modal")}
 let(aw,ah,after)=pixels(base+"after.png");precondition(w==aw && h==ah)
 let changed=zip(before,after).filter {$0 != $1}.count;precondition(changed>200,theme+"-"+mode+" action did not change its frame")
 results[theme+"-"+mode] = ["width":w,"height":h,"blocked_save_checked":(mode.hasPrefix("dialog") || mode.hasPrefix("delete") || mode.hasPrefix("insert")),"dismissed_changed_channels":changed]
}}
let data=try JSONSerialization.data(withJSONObject:results,options:[.prettyPrinted,.sortedKeys])
try data.write(to:URL(fileURLWithPath:"/tmp/next-gui-view-keys-v1-native-keys/pixel-comparison.json"));print(String(data:data,encoding:.utf8)!)
