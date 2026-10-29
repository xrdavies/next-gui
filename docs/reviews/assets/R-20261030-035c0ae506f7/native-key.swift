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
 case "open":key(109,.maskShift)
 case "expressions":key(119);key(124);key(119)
 case "comparison":key(115);for _ in 0..<10 {key(125)};key(36)
 case "regexp":key(36)
 case "number":text("3")
 case "word":for c in "alp" {text(String(c))}
 case "undo":key(6,.maskCommand)
 case "redo":key(6,[.maskCommand,.maskShift])
 case "clear":key(0,.maskCommand);key(51)
 default:fatalError("Expected owned filter menu action")
}
RunLoop.current.run(until:Date(timeIntervalSinceNow:1.0))
print("Owned filter action completed")
