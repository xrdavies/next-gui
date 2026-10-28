import AppKit
import CoreGraphics
import Foundation
guard CommandLine.arguments.count == 3, let pid=Int32(CommandLine.arguments[1]), NSRunningApplication(processIdentifier:pid)?.localizedName == "e28_db_browser" else { fatalError("Expected owned e28 process") }
let source=CGEventSource(stateID:.privateState)!
func send(_ key:CGKeyCode,_ down:Bool,_ flags:CGEventFlags=[]) {
 let event=CGEvent(keyboardEventSource:source,virtualKey:key,keyDown:down)!
 event.flags=flags;event.postToPid(pid)
 RunLoop.current.run(until:Date(timeIntervalSinceNow:0.08))
}
if CommandLine.arguments[2] == "copy-result" {
 let board=NSPasteboard.general
 let originalCount=board.changeCount
 let saved=(board.pasteboardItems ?? []).map { item -> NSPasteboardItem in
  let clone=NSPasteboardItem()
  for type in item.types {guard let data=item.data(forType:type) else {fatalError("Cannot preserve clipboard type")};precondition(clone.setData(data,forType:type))}
  return clone
 }
 precondition(board.changeCount==originalCount,"Clipboard changed before owned test")
 let marker="next-gui-owned-cell-"+UUID().uuidString
 board.clearContents();precondition(board.setString(marker,forType:.string))
 send(55,true,.maskCommand);send(0,true,.maskCommand);send(0,false,.maskCommand);send(55,false)
 send(55,true,.maskCommand);send(8,true,.maskCommand);send(8,false,.maskCommand);send(55,false)
 RunLoop.current.run(until:Date(timeIntervalSinceNow:0.7))
 let afterCount=board.changeCount
 let copied=board.string(forType:.string)
 let owned=copied=="中文A • 18" || copied==marker
 var restored=false
 if owned && board.changeCount==afterCount {board.clearContents();if !saved.isEmpty {precondition(board.writeObjects(saved))};restored=true}
 precondition(copied=="中文A • 18","Owned evaluated cell copy did not match")
 precondition(restored,"Clipboard changed during owned test; left untouched")
 print("Exact evaluated cell text copied; original clipboard restored")
 exit(0)
}
switch CommandLine.arguments[2] {
case "apply":send(55,true,.maskCommand);send(36,true,.maskCommand);send(36,false,.maskCommand);send(55,false)
case "undo":send(55,true,.maskCommand);send(6,true,.maskCommand);send(6,false,.maskCommand);send(55,false)
case "redo":send(55,true,.maskCommand);send(56,true,[.maskCommand,.maskShift]);send(6,true,[.maskCommand,.maskShift]);send(6,false,[.maskCommand,.maskShift]);send(56,false,.maskCommand);send(55,false)
default:fatalError("Unknown owned QA command")
}
RunLoop.current.run(until:Date(timeIntervalSinceNow:0.6))
print("Targeted owned process \(pid) command \(CommandLine.arguments[2])")
