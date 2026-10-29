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
 case "auto":for c in "alp" {text(String(c))}
 case "escape":for c in "A!%B" {text(String(c))}
 case "clear":key(0,.maskCommand);key(51)
 default:fatalError("Expected owned filter action")
}
RunLoop.current.run(until:Date(timeIntervalSinceNow:1.0))
print("Owned filter action completed")
