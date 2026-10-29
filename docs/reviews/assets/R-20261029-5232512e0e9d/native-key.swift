import AppKit
import CoreGraphics
import Foundation
guard CommandLine.arguments.count==3,let pid=Int32(CommandLine.arguments[1]),NSRunningApplication(processIdentifier:pid)?.localizedName=="e28_db_browser" else {fatalError("Expected owned browser")}
let source=CGEventSource(stateID:.privateState)!
func key(_ code:CGKeyCode,_ flags:CGEventFlags=[]) {
 for down in [true,false] {let event=CGEvent(keyboardEventSource:source,virtualKey:code,keyDown:down)!;event.flags=flags;event.postToPid(pid);RunLoop.current.run(until:Date(timeIntervalSinceNow:0.08))}
 RunLoop.current.run(until:Date(timeIntervalSinceNow:0.3))
}
func text(_ value:String) {
 let units=Array(value.utf16)
 for down in [true,false] {
  let event=CGEvent(keyboardEventSource:source,virtualKey:0,keyDown:down)!
  units.withUnsafeBufferPointer {event.keyboardSetUnicodeString(stringLength:units.count,unicodeString:$0.baseAddress!)}
  event.flags=[];event.postToPid(pid);RunLoop.current.run(until:Date(timeIntervalSinceNow:0.08))
 }
 RunLoop.current.run(until:Date(timeIntervalSinceNow:0.6))
}
switch CommandLine.arguments[2] {
 case "select":key(0,.maskCommand)
 case "replace-text":text("x")
 case "replace-hex":text("a")
 case "replace-ascii":text("Z")
 case "ascii":key(48)
 case "undo":key(6,.maskCommand)
 case "redo":key(6,[.maskCommand,.maskShift])
 default:fatalError("Expected owned metadata action")
}
print("Owned native metadata action completed")
