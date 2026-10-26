use std::time::Instant;
/// Same-side pure Ctrl taps; release-to-next-press interval, repeats and chords excluded.
#[derive(Default)]
pub(crate) struct Gesture{
    down:Option<(usize,u64,Instant,bool)>,previous:Option<(usize,Instant,u64)>,
}
impl Gesture{
    pub fn reset(&mut self){*self=Self::default();}
    pub fn observe(&mut self,key:usize,down:bool,repeat:bool,epoch:u64,pure:bool,interval:u32)->bool{
        self.at(key,down,repeat,epoch,pure,interval,Instant::now())
    }
    pub(crate) fn at(&mut self,key:usize,down:bool,repeat:bool,epoch:u64,pure:bool,interval:u32,now:Instant)->bool{
        if ![0xa2,0xa3].contains(&key)||!pure{self.reset();return false;}
        if down{
            if let Some((side,start,_,_))=self.down{if side!=key||start!=epoch{self.reset();}return false;}
            if repeat{self.reset();return false;}
            let second=self.previous.take().is_some_and(|(side,released,activity)|side==key&&activity==epoch&&now.duration_since(released).as_millis()<=interval as u128);
            self.down=Some((key,epoch,now,second));return false;
        }
        let Some((side,start,pressed,second))=self.down.take() else{return false;};
        if side!=key||start!=epoch||now.duration_since(pressed).as_millis()>600{self.reset();return false;}
        if second{self.previous=None;return true;}
        self.previous=Some((key,now,epoch));false
    }
}
